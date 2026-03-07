//! LLM Gateway - OpenAI-compatible API with local + premium routing.
//! Config loaded from ~/.llm-gateway/config.toml or env vars.

mod batch;
mod batch_worker;
mod config;
mod db;
mod providers;
mod request_context;
mod routing;
mod types;
mod utils;
use batch::{create_batch_job, BatchQueue, BatchSubmitRequest};
use batch_worker::spawn_batch_worker;
use config::*;
use db::*;
use providers::{call_anthropic, call_gemini, call_local_ollama, call_openai};
use routing::{
    candidate_key, classify_task, code_candidates, dedupe_profiles, execution_budget,
    fast_candidates, genius_candidates, is_retryable_error, parse_mode, premium_profiles,
    route_mode_name, score_local_model, task_name,
};
use types::*;
use utils::{error_response, error_response_with_request_id};

use axum::{
    extract::{Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::hash_map::DefaultHasher,
    collections::HashMap,
    convert::Infallible,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::atomic::AtomicBool,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};
use uuid::Uuid;

/// Emit fuel event to Supabase for per-caller billing.
///
/// This inserts into the fuel_events table, which is an append-only ledger
/// used for monthly billing reports. Each event has an idempotency key
/// to prevent duplicate billing.
async fn emit_fuel_to_supabase(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    identity: &ClientIdentity,
    delta: &FuelDelta,
    metadata: serde_json::Value,
) -> anyhow::Result<FuelEmitResult> {
    // Skip if not billable to Supabase
    if !identity.is_supabase_billable() {
        return Ok(FuelEmitResult::default());
    }

    let url = config
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase URL not configured"))?;
    let key = config
        .service_role_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase service_role_key not configured"))?;

    let tenant_id = identity.tenant_id.as_ref().unwrap();
    let app_id = identity.app_id.as_ref().unwrap();
    let user_id = identity
        .user_id
        .as_ref()
        .map(|s| s.as_str())
        .or_else(|| identity.app_id.as_ref().map(|s| s.as_str()))
        .unwrap_or(tenant_id);

    // Generate idempotency key from request fingerprint
    let idempotency_key = format!(
        "llm-gateway:{}:{}:{}",
        app_id,
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        uuid::Uuid::new_v4()
    );

    // Calculate total tokens as the unit for billing
    let total_tokens = delta.prompt_tokens + delta.completion_tokens;
    if total_tokens == 0 {
        return Ok(FuelEmitResult::default());
    }

    if let Err(reason) = validate_fuel_metadata(&metadata, "llm_tokens") {
        warn!(
            reason = %reason,
            tenant_id = %tenant_id,
            app_id = %app_id,
            "fuel.emit.invalid"
        );
        return Err(anyhow::anyhow!("invalid fuel metadata: {}", reason));
    }

    let fuel_event = json!({
        "idempotency_key": idempotency_key,
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "units": total_tokens,
        "unit_type": "llm_tokens",
        "source": "llm-gateway",
        "metadata": metadata,
    });

    let resp = client
        .post(format!("{}/rest/v1/fuel_events?select=event_id", url))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=representation")
        .json(&fuel_event)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Failed to emit fuel event to Supabase");
        return Err(anyhow::anyhow!(
            "Supabase fuel insert failed: {} - {}",
            status,
            body
        ));
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_else(|_| json!([]));
    let event_id = body
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("event_id"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    Ok(FuelEmitResult { event_id })
}

fn validate_fuel_metadata(metadata: &serde_json::Value, unit_type: &str) -> Result<(), String> {
    let Some(map) = metadata.as_object() else {
        return Err("metadata must be a JSON object".into());
    };

    for key in ["event_type", "trace_id", "outcome"] {
        let Some(value) = map.get(key).and_then(serde_json::Value::as_str) else {
            return Err(format!("missing or invalid metadata.{key}"));
        };
        if value.trim().is_empty() {
            return Err(format!("metadata.{key} cannot be empty"));
        }
    }

    if !map.contains_key("parent_event_id") {
        return Err("missing metadata.parent_event_id".into());
    }
    if !map
        .get("parent_event_id")
        .map(|value| value.is_null() || value.is_string())
        .unwrap_or(false)
    {
        return Err("metadata.parent_event_id must be string|null".into());
    }

    if unit_type == "llm_tokens" {
        for key in ["provider", "model"] {
            let Some(value) = map.get(key).and_then(serde_json::Value::as_str) else {
                return Err(format!("missing or invalid metadata.{key}"));
            };
            if value.trim().is_empty() {
                return Err(format!("metadata.{key} cannot be empty"));
            }
        }

        for key in ["prompt_tokens", "completion_tokens", "latency_ms"] {
            if !map
                .get(key)
                .map(serde_json::Value::is_number)
                .unwrap_or(false)
            {
                return Err(format!("missing or invalid metadata.{key}"));
            }
        }
    }

    Ok(())
}

/// Log LLM request telemetry to Supabase.
///
/// This inserts into the llm_requests table for analytics and debugging.
/// Unlike fuel_events, this is not used for billing - it stores detailed
/// request metadata like provider, model, latency, and error messages.
async fn log_llm_request_to_supabase(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    identity: &ClientIdentity,
    request_id: &str,
    trace_id: &str,
    plan_id: Option<&str>,
    ci_target: Option<&str>,
    fallback_behavior: Option<&str>,
    provider: &str,
    model: &str,
    mode: &str,
    input_tokens: u32,
    output_tokens: u32,
    latency_ms: u32,
    success: bool,
    fallback_used: bool,
    fuel_event_id: Option<&str>,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    // Skip if not billable (no tenant/app context)
    if !identity.is_supabase_billable() {
        return Ok(());
    }

    let url = config
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase URL not configured"))?;
    let key = config
        .service_role_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase service_role_key not configured"))?;

    let tenant_id = identity.tenant_id.as_ref().unwrap();
    let app_id = identity.app_id.as_ref().unwrap();
    let user_id = identity.user_id.as_ref();

    let request_log = json!({
        "request_id": request_id,
        "trace_id": trace_id,
        "plan_id": plan_id,
        "ci_target": ci_target,
        "fallback_behavior": fallback_behavior,
        "fallback_used": fallback_used,
        "fuel_event_id": fuel_event_id,
        "source": "llm-gateway",
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "provider": provider,
        "model": model,
        "mode": mode,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "latency_ms": latency_ms,
        "success": success,
        "error_message": error_message,
    });

    let resp = client
        .post(format!("{}/rest/v1/llm_requests", url))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&request_log)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Failed to log LLM request to Supabase");
        // Don't return error - logging failure shouldn't block the request
    }

    Ok(())
}

async fn emit_obs_event(
    client: &reqwest::Client,
    config: &ObsApiConfig,
    event: serde_json::Value,
) -> anyhow::Result<()> {
    let Some(base_url) = config.base_url.as_deref() else {
        return Ok(());
    };

    let url = format!("{}/api/v1/events/ingest", base_url.trim_end_matches('/'));
    let mut req = client.post(url).json(&event);
    if let Some(token) = config.token.as_deref() {
        req = req.bearer_auth(token);
    }

    let response = req.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("obs ingest failed: {} {}", status, body));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct OnboardingSyncRequest {
    app_name: Option<String>,
    rotate: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OnboardingClaims {
    sub: String,
    app_name: Option<String>,
    aud: Option<String>,
    exp: usize,
}

/// JWT claims from Supabase access token
///
/// Supabase JWTs contain custom claims under `app_metadata` with tenant/app info.
/// The `sub` field is the Supabase user UUID.
///
/// Service tokens have a different format:
/// - `sub` is the app_id
/// - `tenant_id` is at top level
/// - `role` = "service"
#[derive(Debug, Deserialize)]
struct SupabaseJwtClaims {
    /// Supabase user UUID (for user tokens) or app_id (for service tokens)
    sub: String,
    /// Token expiration (Unix timestamp)
    exp: usize,
    /// Audience (usually the Supabase project URL)
    aud: Option<String>,
    /// App metadata with custom claims (tenant_id, app_id) - for user tokens
    #[serde(default)]
    app_metadata: SupabaseAppMetadata,
    /// Top-level tenant_id (for service tokens)
    tenant_id: Option<String>,
    /// Role: "service" for service tokens, absent for user tokens
    role: Option<String>,
    /// Optional OAuth-like scope claim (space/comma separated values)
    scope: Option<String>,
    /// Optional alternative scope field name
    scopes: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct SupabaseAppMetadata {
    /// Tenant UUID from onboarding
    tenant_id: Option<String>,
    /// App UUID from onboarding
    app_id: Option<String>,
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(ToString::to_string)
}

fn scope_contains(scope_value: &str, required_scope: &str) -> bool {
    let required = required_scope.trim().to_ascii_lowercase();
    if required.is_empty() {
        return true;
    }

    scope_value
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .any(|s| s == required)
}

fn header_optional(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

/// Try to validate token as a Supabase JWT (user or service token)
fn try_supabase_jwt(token: &str, config: &SupabaseConfig) -> Option<ClientIdentity> {
    let secret = config.jwt_secret.as_deref()?;

    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    // Supabase JWTs may not have audience, so we only validate if configured
    if let Some(aud) = &config.jwt_audience {
        validation.set_audience(&[aud.as_str()]);
    } else {
        validation.validate_aud = false;
    }

    let decoded = jsonwebtoken::decode::<SupabaseJwtClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()?;

    let claims = decoded.claims;

    // Check if this is a service token (role=service)
    if claims.role.as_deref() == Some("service") {
        if let Some(required_scope) = config
            .required_service_scope
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let granted = claims
                .scope
                .as_deref()
                .or(claims.scopes.as_deref())
                .unwrap_or("");
            if !scope_contains(granted, required_scope) {
                return None;
            }
        }

        // Service token: sub=app_id, tenant_id at top level
        let tenant_id = claims.tenant_id?;
        let app_id = claims.sub.clone();
        return Some(ClientIdentity::from_service_token(tenant_id, app_id));
    }

    // User token: tenant_id and app_id in app_metadata
    let tenant_id = claims.app_metadata.tenant_id?;
    let app_id = claims.app_metadata.app_id?;

    Some(ClientIdentity::from_supabase_jwt(
        tenant_id,
        app_id,
        Some(claims.sub),
    ))
}

async fn authenticate_client(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<ClientIdentity, (StatusCode, Json<ErrorResponse>)> {
    let token = bearer_token(headers).ok_or_else(|| {
        error_response(
            StatusCode::UNAUTHORIZED,
            "Missing API key",
            "invalid_request_error",
        )
    })?;

    // Extract x-calling-app header (identifies which ecosystem app originated the request)
    let calling_app = headers
        .get("x-calling-app")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);

    let allow_jwt = state.config.security.legacy_api_key_mode != "legacy_only";
    let (allow_legacy, legacy_block_reason) = legacy_api_key_allowed(&state.config.security);

    // 1. Try Supabase JWT auth first (unless explicitly forcing legacy-only mode)
    if allow_jwt {
        if let Some(identity) = try_supabase_jwt(&token, &state.config.supabase) {
            info!(
                tenant_id = ?identity.tenant_id,
                app_id = ?identity.app_id,
                calling_app = ?calling_app,
                "Supabase JWT authenticated"
            );
            return Ok(identity.with_calling_app(calling_app));
        }
    }

    // 2. Check if it's the gateway's admin API key
    if token == state.config.api_key {
        return Ok(ClientIdentity::admin().with_calling_app(calling_app));
    }

    // 3. Fall back to local SQLite API key lookup (legacy clients)
    if allow_legacy {
        let resolved = resolve_client_by_api_key(&state.fuel_db_path, &token).map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("auth db error: {e}"),
                "upstream_error",
            )
        })?;
        if let Some((client_id, app_name)) = resolved {
            return Ok(
                ClientIdentity::from_api_key(client_id, app_name).with_calling_app(calling_app)
            );
        }
    }

    if let Some(reason) = legacy_block_reason {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            &format!("Invalid credentials; {reason}. Use Supabase JWT."),
            "invalid_request_error",
        ));
    }

    Err(error_response(
        StatusCode::UNAUTHORIZED,
        "Invalid API key or JWT",
        "invalid_request_error",
    ))
}

#[derive(Clone, Copy, Debug)]
struct RateLimitWindow {
    started_at: Instant,
    count: u32,
}

fn rate_limit_key(client: &ClientIdentity) -> String {
    if let Some(app_id) = &client.app_id {
        return format!("app:{app_id}");
    }
    if client.client_id > 0 {
        return format!("legacy:{}", client.client_id);
    }
    format!("name:{}", client.app_name)
}

async fn enforce_rate_limit(
    state: &AppState,
    client: &ClientIdentity,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if client.app_name == "admin" {
        return Ok(());
    }

    let limit = state.config.security.rate_limit_per_minute.max(1);
    let now = Instant::now();
    let window = Duration::from_secs(60);
    let mut guard = state.rate_limits.lock().await;

    // Opportunistic cleanup to avoid unbounded map growth.
    if guard.len() > 10_000 {
        let cutoff = Duration::from_secs(300);
        guard.retain(|_, v| now.duration_since(v.started_at) <= cutoff);
    }

    let entry = guard
        .entry(rate_limit_key(client))
        .or_insert(RateLimitWindow {
            started_at: now,
            count: 0,
        });

    if now.duration_since(entry.started_at) >= window {
        entry.started_at = now;
        entry.count = 0;
    }

    if entry.count >= limit {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded",
            "rate_limit_error",
        ));
    }

    entry.count = entry.count.saturating_add(1);
    Ok(())
}

fn verify_onboarding_jwt(
    token: &str,
    security: &SecurityPolicy,
) -> Result<OnboardingClaims, (StatusCode, Json<ErrorResponse>)> {
    let secret = security.onboarding_jwt_secret.as_deref().ok_or_else(|| {
        error_response(
            StatusCode::BAD_GATEWAY,
            "CLI JWT secret not configured",
            "provider_auth_error",
        )
    })?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    if let Some(aud) = &security.onboarding_jwt_audience {
        validation.set_audience(&[aud.as_str()]);
    }
    let decoded = jsonwebtoken::decode::<OnboardingClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        error_response(
            StatusCode::UNAUTHORIZED,
            &format!("invalid onboarding JWT: {e}"),
            "invalid_request_error",
        )
    })?;
    Ok(decoded.claims)
}

struct AppState {
    config: Config,
    client: reqwest::Client,
    route_health: RwLock<HashMap<String, RouteHealth>>,
    local_inflight: Arc<Semaphore>,
    metrics: GatewayMetrics,
    fuel_db_path: String,
    /// Request deduplication cache (hash -> (response, timestamp))
    request_cache: RwLock<HashMap<u64, (String, Instant)>>,
    /// In-memory fixed-window limiter by client/app identity
    rate_limits: Mutex<HashMap<String, RateLimitWindow>>,
    /// Batch processing queue for non-urgent requests (50% discount)
    batch_queue: BatchQueue,
}

const REQUEST_ID_HEADER: &str = "x-request-id";
const RESPONSE_ENVELOPE_SCHEMA: &str =
    "https://logline.world/schemas/response-envelope.v1.schema.json";
const MODE_CONTRACT_VERSION: &str = "2026-03-05";
const CODE247_CONTRACT_VERSION: &str = "2026-03-06";
const CANONICAL_MODES: [&str; 3] = ["genius", "fast", "code"];
const MODE_ALIASES: [(&str, &str); 3] =
    [("premium", "genius"), ("local", "code"), ("auto", "code")];

#[derive(Debug, Clone, Default)]
struct FuelEmitResult {
    event_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseManifestRequest {
    release_version: Option<String>,
    notes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CloudReconcileRequest {
    from: Option<String>,
    to: Option<String>,
    dry_run: Option<bool>,
    max_events: Option<u32>,
}

#[derive(Debug, Clone)]
struct FuelEventForSettlement {
    event_id: String,
    occurred_at: chrono::DateTime<chrono::Utc>,
    units: f64,
    provider: String,
    model: String,
    request_id: Option<String>,
}

#[derive(Debug, Clone)]
struct SettlementAggregate {
    usage_tokens: f64,
    settled_usd: f64,
}

fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

async fn request_id_middleware(request: Request, next: Next) -> Response {
    let request_id = request_id_from_headers(request.headers());
    let mut response =
        request_context::with_request_id_scope(request_id.clone(), next.run(request)).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static(REQUEST_ID_HEADER), value);
    }
    response
}

fn success_envelope_json(
    request_id: String,
    payload: serde_json::Value,
) -> Json<serde_json::Value> {
    let body = match payload {
        serde_json::Value::Object(mut map) => {
            map.insert(
                "request_id".to_string(),
                serde_json::Value::String(request_id.clone()),
            );
            map.insert(
                "output_schema".to_string(),
                serde_json::Value::String(RESPONSE_ENVELOPE_SCHEMA.to_string()),
            );
            serde_json::Value::Object(map)
        }
        value => json!({
            "request_id": request_id,
            "output_schema": RESPONSE_ENVELOPE_SCHEMA,
            "data": value
        }),
    };
    Json(body)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalAdaptiveProfile {
    Baseline,
    Degraded,
    Emergency,
}

impl LocalAdaptiveProfile {
    fn as_str(self) -> &'static str {
        match self {
            LocalAdaptiveProfile::Baseline => "baseline",
            LocalAdaptiveProfile::Degraded => "degraded",
            LocalAdaptiveProfile::Emergency => "emergency",
        }
    }
}

fn local_quantiles_for_adaptive_tuning(snapshot: &LatencyQuantilesSnapshot) -> LatencyQuantiles {
    snapshot
        .by_provider
        .get("local")
        .cloned()
        .or_else(|| snapshot.by_mode.get("code").cloned())
        .unwrap_or_else(|| snapshot.all.clone())
}

fn select_local_adaptive_profile(
    policy: &LocalPolicy,
    quantiles: &LatencyQuantiles,
) -> LocalAdaptiveProfile {
    if !policy.adaptive_tuning_enabled || quantiles.samples < policy.adaptive_min_samples {
        return LocalAdaptiveProfile::Baseline;
    }
    if quantiles.p99_ms >= policy.adaptive_p99_emergency_ms {
        return LocalAdaptiveProfile::Emergency;
    }
    if quantiles.p95_ms >= policy.adaptive_p95_degraded_ms {
        return LocalAdaptiveProfile::Degraded;
    }
    LocalAdaptiveProfile::Baseline
}

fn cap_option_u32(current: Option<u32>, cap: u32) -> Option<u32> {
    Some(current.unwrap_or(cap).min(cap))
}

fn apply_local_adaptive_profile(
    policy: &LocalPolicy,
    base: &LocalRequestParams,
    profile: LocalAdaptiveProfile,
) -> LocalRequestParams {
    let mut tuned = base.clone();
    match profile {
        LocalAdaptiveProfile::Baseline => {}
        LocalAdaptiveProfile::Degraded => {
            tuned.options.num_ctx =
                cap_option_u32(tuned.options.num_ctx, policy.adaptive_degraded_num_ctx_cap);
            tuned.options.num_batch = cap_option_u32(
                tuned.options.num_batch,
                policy.adaptive_degraded_num_batch_cap,
            );
        }
        LocalAdaptiveProfile::Emergency => {
            tuned.options.num_ctx =
                cap_option_u32(tuned.options.num_ctx, policy.adaptive_emergency_num_ctx_cap);
            tuned.options.num_batch = cap_option_u32(
                tuned.options.num_batch,
                policy.adaptive_emergency_num_batch_cap,
            );
        }
    }
    tuned
}

fn local_max_tokens_for_profile(
    policy: &LocalPolicy,
    profile: LocalAdaptiveProfile,
    requested: Option<u32>,
) -> u32 {
    if let Some(v) = requested {
        return v.max(1);
    }
    match profile {
        LocalAdaptiveProfile::Baseline => policy.default_max_tokens.max(1),
        LocalAdaptiveProfile::Degraded => policy.adaptive_degraded_max_tokens.max(1),
        LocalAdaptiveProfile::Emergency => policy.adaptive_emergency_max_tokens.max(1),
    }
}

fn estimate_local_energy(
    policy: &LocalPolicy,
    latency_ms: u64,
    has_timing_signal: bool,
) -> (f64, f64, f64) {
    let energy_kwh = ((latency_ms as f64) * policy.energy_model_watts) / 3_600_000_000_f64;
    let carbon_gco2e = energy_kwh * policy.energy_carbon_intensity_gco2e_per_kwh;
    let confidence = (policy.energy_confidence_base
        + if has_timing_signal {
            policy.energy_confidence_timing_bonus
        } else {
            0.0
        })
    .clamp(0.0, 1.0);
    (energy_kwh.max(0.0), carbon_gco2e.max(0.0), confidence)
}

fn legacy_api_key_allowed(policy: &SecurityPolicy) -> (bool, Option<String>) {
    match policy.legacy_api_key_mode.as_str() {
        "disabled" => (false, Some("legacy API keys are disabled".into())),
        _ => {
            let Some(sunset) = policy.legacy_api_key_sunset_at.as_deref() else {
                return (true, None);
            };
            match chrono::DateTime::parse_from_rfc3339(sunset) {
                Ok(ts) => {
                    if chrono::Utc::now() >= ts.with_timezone(&chrono::Utc) {
                        (
                            false,
                            Some(format!(
                                "legacy API keys sunset reached at {}",
                                ts.to_rfc3339()
                            )),
                        )
                    } else {
                        (true, None)
                    }
                }
                Err(_) => (true, None),
            }
        }
    }
}

fn local_queue_wait_for_profile(policy: &LocalPolicy, profile: LocalAdaptiveProfile) -> Duration {
    let wait_ms = match profile {
        LocalAdaptiveProfile::Baseline => policy.max_queue_wait_ms,
        LocalAdaptiveProfile::Degraded => policy
            .adaptive_degraded_queue_wait_ms
            .min(policy.max_queue_wait_ms),
        LocalAdaptiveProfile::Emergency => policy
            .adaptive_emergency_queue_wait_ms
            .min(policy.adaptive_degraded_queue_wait_ms)
            .min(policy.max_queue_wait_ms),
    };
    Duration::from_millis(wait_ms.max(50))
}

fn local_warmup_schedule_for_profile(
    policy: &LocalPolicy,
    profile: LocalAdaptiveProfile,
) -> (Duration, Duration) {
    let interval_secs = match profile {
        LocalAdaptiveProfile::Baseline => policy.warmup_interval_secs,
        LocalAdaptiveProfile::Degraded => (policy.warmup_interval_secs / 2).max(15),
        LocalAdaptiveProfile::Emergency => (policy.warmup_interval_secs / 4).max(15),
    };
    let timeout_ms = match profile {
        LocalAdaptiveProfile::Baseline => policy.warmup_timeout_ms,
        LocalAdaptiveProfile::Degraded => (policy.warmup_timeout_ms * 3 / 4).max(1000),
        LocalAdaptiveProfile::Emergency => (policy.warmup_timeout_ms / 2).max(1000),
    };
    (
        Duration::from_secs(interval_secs.max(15)),
        Duration::from_millis(timeout_ms.max(1000)),
    )
}

async fn resolve_local_adaptive_profile(state: &AppState) -> LocalAdaptiveProfile {
    let snapshot = state.metrics.latency_quantiles_snapshot();
    let quantiles = local_quantiles_for_adaptive_tuning(&snapshot);
    select_local_adaptive_profile(&state.config.local, &quantiles)
}

fn latency_quantiles_json(quantiles: &LatencyQuantiles) -> serde_json::Value {
    json!({
        "samples": quantiles.samples,
        "p50_ms": quantiles.p50_ms,
        "p95_ms": quantiles.p95_ms,
        "p99_ms": quantiles.p99_ms
    })
}

fn latency_quantiles_map_json(values: &HashMap<String, LatencyQuantiles>) -> serde_json::Value {
    let mut keys = values.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    let mut map = serde_json::Map::new();
    for key in keys {
        if let Some(value) = values.get(&key) {
            map.insert(key, latency_quantiles_json(value));
        }
    }
    serde_json::Value::Object(map)
}

async fn health(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    success_envelope_json(
        request_id,
        json!({
            "ok": true,
            "service": "llm-gateway",
            "version": "0.3.0",
            "default_mode": state.config.default_mode,
            "premium": {
                "openai_enabled": provider_ready(&state.config.premium.openai),
                "anthropic_enabled": provider_ready(&state.config.premium.anthropic),
                "gemini_enabled": provider_ready(&state.config.premium.gemini),
            }
        }),
    )
}

async fn list_models(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let profiles = fast_model_matrix(&state.config);
    let data = profiles
        .into_iter()
        .map(|m| ModelInfo {
            id: m.model,
            object: "model".into(),
            created: 1700000000,
            owned_by: m.provider,
        })
        .collect();
    Ok(success_envelope_json(
        request_id,
        json!(ModelsResponse {
            object: "list".into(),
            data,
        }),
    ))
}

async fn matrix(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let profiles = fast_model_matrix(&state.config);
    Ok(success_envelope_json(
        request_id,
        json!(MatrixResponse {
            updated_at: chrono::Utc::now().to_rfc3339(),
            models: profiles,
        }),
    ))
}

async fn mode_contract(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    let mut alias_map = serde_json::Map::new();
    for (legacy, canonical) in MODE_ALIASES {
        alias_map.insert(
            legacy.to_string(),
            serde_json::Value::String(canonical.to_string()),
        );
    }
    success_envelope_json(
        request_id,
        json!({
            "contract_version": MODE_CONTRACT_VERSION,
            "default_mode": state.config.default_mode,
            "canonical_modes": CANONICAL_MODES,
            "legacy_aliases": serde_json::Value::Object(alias_map),
            "notes": [
                "Send genius, fast, or code for forward compatibility",
                "Legacy aliases remain accepted for backward compatibility"
            ]
        }),
    )
}

async fn code247_contract(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    success_envelope_json(
        request_id,
        json!({
            "contract_version": CODE247_CONTRACT_VERSION,
            "source_service": "llm-gateway",
            "chat_endpoint": "/v1/chat/completions",
            "modes_endpoint": "/v1/modes",
            "canonical_modes": CANONICAL_MODES,
            "request_headers": {
                "required": ["authorization"],
                "optional": [
                    "x-request-id",
                    "x-calling-app",
                    "x-plan-id",
                    "x-intention-id",
                    "x-run-id",
                    "x-issue-id",
                    "x-pr-id",
                    "x-deploy-id",
                    "x-ci-target",
                    "x-fallback-behavior"
                ]
            },
            "defaults": {
                "ci_target": "code247-ci/main",
                "fallback_behavior": "provider-fallback-with-timeout",
                "mode": state.config.default_mode
            },
            "response_contract": {
                "success_envelope_fields": ["request_id", "output_schema"],
                "error_envelope_fields": ["request_id", "output_schema", "error.message", "error.type", "error.code"],
                "chat_schema": "https://logline.world/schemas/llm-gateway.chat-response.v1.schema.json",
                "error_schema": "https://logline.world/schemas/error-envelope.v1.schema.json"
            }
        }),
    )
}

async fn metrics_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    let latency_quantiles = state.metrics.latency_quantiles_snapshot();
    let selected_by_provider = state.metrics.selected_by_provider.read().await.clone();
    let selected_by_model = state.metrics.selected_by_model.read().await.clone();
    let error_by_provider = state.metrics.error_by_provider.read().await.clone();
    let error_by_model = state.metrics.error_by_model.read().await.clone();
    let local_adaptive_profiles = state.metrics.local_adaptive_profile_snapshot().await;

    let prompt = state
        .metrics
        .estimated_prompt_tokens_total
        .load(Ordering::Relaxed);
    let completion = state
        .metrics
        .estimated_completion_tokens_total
        .load(Ordering::Relaxed);

    success_envelope_json(
        request_id,
        json!({
            "service": "llm-gateway",
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "uptime_seconds": state.metrics.started.elapsed().as_secs(),
            "requests": {
                "total": state.metrics.total_requests.load(Ordering::Relaxed),
                "success": state.metrics.success_requests.load(Ordering::Relaxed),
                "failed": state.metrics.error_requests.load(Ordering::Relaxed),
                "stream": state.metrics.stream_requests.load(Ordering::Relaxed),
                "non_stream": state.metrics.non_stream_requests.load(Ordering::Relaxed),
                "fallback_attempt_failures": state.metrics.fallback_attempt_failures.load(Ordering::Relaxed)
            },
            "latency_ms": {
                "all": latency_quantiles_json(&latency_quantiles.all),
                "by_mode": latency_quantiles_map_json(&latency_quantiles.by_mode),
                "by_provider": latency_quantiles_map_json(&latency_quantiles.by_provider),
                "by_model": latency_quantiles_map_json(&latency_quantiles.by_model)
            },
            "selection": {
                "by_provider": selected_by_provider,
                "by_model": selected_by_model
            },
            "errors": {
                "by_provider": error_by_provider,
                "by_model": error_by_model
            },
            "local": {
                "requests_total": state.metrics.local_requests_total.load(Ordering::Relaxed),
                "queue_wait_ms_total": state.metrics.local_queue_wait_ms_total.load(Ordering::Relaxed),
                "queue_timeouts_total": state.metrics.local_queue_timeouts_total.load(Ordering::Relaxed),
                "warmup_runs_total": state.metrics.local_warmup_runs_total.load(Ordering::Relaxed),
                "warmup_failures_total": state.metrics.local_warmup_failures_total.load(Ordering::Relaxed),
                "ollama_timing_samples_total": state.metrics.local_ollama_timing_samples_total.load(Ordering::Relaxed),
                "adaptive_profile_total": local_adaptive_profiles,
                "latency_targets_ms": {
                    "p95_degraded": state.config.local.adaptive_p95_degraded_ms,
                    "p99_emergency": state.config.local.adaptive_p99_emergency_ms
                }
            },
            "fuel_pipeline": {
                "supabase_emit_success_total": state.metrics.fuel_supabase_emit_success_total.load(Ordering::Relaxed),
                "supabase_emit_fail_total": state.metrics.fuel_supabase_emit_fail_total.load(Ordering::Relaxed),
                "sqlite_fallback_writes_total": state.metrics.fuel_sqlite_fallback_writes_total.load(Ordering::Relaxed),
                "settlement_runs_total": state.metrics.fuel_settlement_runs_total.load(Ordering::Relaxed),
                "settlement_failures_total": state.metrics.fuel_settlement_failures_total.load(Ordering::Relaxed),
                "settled_events_total": state.metrics.fuel_settled_events_total.load(Ordering::Relaxed),
                "local_energy_updates_total": state.metrics.fuel_local_energy_updates_total.load(Ordering::Relaxed)
            },
            "cost": {
                "estimated_prompt_tokens_total": prompt,
                "estimated_completion_tokens_total": completion,
                "estimated_tokens_total": prompt + completion
            }
        }),
    )
}

async fn metrics(State(state): State<Arc<AppState>>) -> Response {
    let now = Instant::now();
    let open_circuits = {
        let guard = state.route_health.read().await;
        guard
            .values()
            .filter(|h| h.open_until.map(|u| u > now).unwrap_or(false))
            .count() as u64
    };
    let provider_counts = state.metrics.selected_by_provider.read().await.clone();
    let model_counts = state.metrics.selected_by_model.read().await.clone();
    let error_by_provider = state.metrics.error_by_provider.read().await.clone();
    let error_by_model = state.metrics.error_by_model.read().await.clone();
    let local_adaptive_profiles = state.metrics.local_adaptive_profile_snapshot().await;
    let latency_quantiles = state.metrics.latency_quantiles_snapshot();
    let mut body = String::new();
    body.push_str("# HELP llm_gateway_uptime_seconds Gateway uptime in seconds\n");
    body.push_str("# TYPE llm_gateway_uptime_seconds gauge\n");
    body.push_str(&format!(
        "llm_gateway_uptime_seconds {}\n",
        state.metrics.started.elapsed().as_secs()
    ));
    body.push_str("# HELP llm_gateway_requests_total Total gateway requests\n");
    body.push_str("# TYPE llm_gateway_requests_total counter\n");
    body.push_str(&format!(
        "llm_gateway_requests_total {}\n",
        state.metrics.total_requests.load(Ordering::Relaxed)
    ));
    body.push_str(&format!(
        "llm_gateway_requests_success_total {}\n",
        state.metrics.success_requests.load(Ordering::Relaxed)
    ));
    body.push_str(&format!(
        "llm_gateway_requests_error_total {}\n",
        state.metrics.error_requests.load(Ordering::Relaxed)
    ));
    body.push_str(&format!(
        "llm_gateway_requests_stream_total {}\n",
        state.metrics.stream_requests.load(Ordering::Relaxed)
    ));
    body.push_str(&format!(
        "llm_gateway_requests_non_stream_total {}\n",
        state.metrics.non_stream_requests.load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_latency_ms_total Sum of request latency in ms\n");
    body.push_str("# TYPE llm_gateway_latency_ms_total counter\n");
    body.push_str(&format!(
        "llm_gateway_latency_ms_total {}\n",
        state.metrics.total_latency_ms.load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_latency_ms_max Max request latency in ms\n");
    body.push_str("# TYPE llm_gateway_latency_ms_max gauge\n");
    body.push_str(&format!(
        "llm_gateway_latency_ms_max {}\n",
        state.metrics.max_latency_ms.load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_latency_ms_p50 Rolling p50 request latency in ms\n");
    body.push_str("# TYPE llm_gateway_latency_ms_p50 gauge\n");
    body.push_str(&format!(
        "llm_gateway_latency_ms_p50{{mode=\"all\"}} {}\n",
        latency_quantiles.all.p50_ms
    ));
    body.push_str("# HELP llm_gateway_latency_ms_p95 Rolling p95 request latency in ms\n");
    body.push_str("# TYPE llm_gateway_latency_ms_p95 gauge\n");
    body.push_str(&format!(
        "llm_gateway_latency_ms_p95{{mode=\"all\"}} {}\n",
        latency_quantiles.all.p95_ms
    ));
    body.push_str("# HELP llm_gateway_latency_ms_p99 Rolling p99 request latency in ms\n");
    body.push_str("# TYPE llm_gateway_latency_ms_p99 gauge\n");
    body.push_str(&format!(
        "llm_gateway_latency_ms_p99{{mode=\"all\"}} {}\n",
        latency_quantiles.all.p99_ms
    ));
    body.push_str("# HELP llm_gateway_latency_samples_total Rolling latency sample count\n");
    body.push_str("# TYPE llm_gateway_latency_samples_total gauge\n");
    body.push_str(&format!(
        "llm_gateway_latency_samples_total{{mode=\"all\"}} {}\n",
        latency_quantiles.all.samples
    ));
    let mut mode_keys = latency_quantiles
        .by_mode
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    mode_keys.sort();
    for mode in mode_keys {
        if let Some(q) = latency_quantiles.by_mode.get(&mode) {
            let safe_mode = mode.replace('"', "_");
            body.push_str(&format!(
                "llm_gateway_latency_ms_p50{{mode=\"{}\"}} {}\n",
                safe_mode, q.p50_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p95{{mode=\"{}\"}} {}\n",
                safe_mode, q.p95_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p99{{mode=\"{}\"}} {}\n",
                safe_mode, q.p99_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_samples_total{{mode=\"{}\"}} {}\n",
                safe_mode, q.samples
            ));
        }
    }
    let mut provider_latency_keys = latency_quantiles
        .by_provider
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    provider_latency_keys.sort();
    for provider in provider_latency_keys {
        if let Some(q) = latency_quantiles.by_provider.get(&provider) {
            let safe_provider = provider.replace('"', "_");
            body.push_str(&format!(
                "llm_gateway_latency_ms_p50{{provider=\"{}\"}} {}\n",
                safe_provider, q.p50_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p95{{provider=\"{}\"}} {}\n",
                safe_provider, q.p95_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p99{{provider=\"{}\"}} {}\n",
                safe_provider, q.p99_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_samples_total{{provider=\"{}\"}} {}\n",
                safe_provider, q.samples
            ));
        }
    }
    let mut model_latency_keys = latency_quantiles
        .by_model
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    model_latency_keys.sort();
    for model in model_latency_keys {
        if let Some(q) = latency_quantiles.by_model.get(&model) {
            let safe_model = model.replace('"', "_");
            body.push_str(&format!(
                "llm_gateway_latency_ms_p50{{model=\"{}\"}} {}\n",
                safe_model, q.p50_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p95{{model=\"{}\"}} {}\n",
                safe_model, q.p95_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_ms_p99{{model=\"{}\"}} {}\n",
                safe_model, q.p99_ms
            ));
            body.push_str(&format!(
                "llm_gateway_latency_samples_total{{model=\"{}\"}} {}\n",
                safe_model, q.samples
            ));
        }
    }
    body.push_str("# HELP llm_gateway_fallback_attempt_failures_total Failed route attempts before fallback\n");
    body.push_str("# TYPE llm_gateway_fallback_attempt_failures_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fallback_attempt_failures_total {}\n",
        state
            .metrics
            .fallback_attempt_failures
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_circuit_breaker_opens_total Circuit breaker open transitions\n",
    );
    body.push_str("# TYPE llm_gateway_circuit_breaker_opens_total counter\n");
    body.push_str(&format!(
        "llm_gateway_circuit_breaker_opens_total {}\n",
        state.metrics.circuit_breaker_opens.load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_circuit_breaker_open_routes Current open circuits\n");
    body.push_str("# TYPE llm_gateway_circuit_breaker_open_routes gauge\n");
    body.push_str(&format!(
        "llm_gateway_circuit_breaker_open_routes {}\n",
        open_circuits
    ));
    body.push_str("# HELP llm_gateway_local_requests_total Total local-route requests\n");
    body.push_str("# TYPE llm_gateway_local_requests_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_requests_total {}\n",
        state.metrics.local_requests_total.load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_queue_wait_ms_total Total wait time on local concurrency limiter\n");
    body.push_str("# TYPE llm_gateway_local_queue_wait_ms_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_queue_wait_ms_total {}\n",
        state
            .metrics
            .local_queue_wait_ms_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_local_queue_timeouts_total Local requests that exceeded queue wait budget\n",
    );
    body.push_str("# TYPE llm_gateway_local_queue_timeouts_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_queue_timeouts_total {}\n",
        state
            .metrics
            .local_queue_timeouts_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_warmup_runs_total Total local warmup attempts\n");
    body.push_str("# TYPE llm_gateway_local_warmup_runs_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_warmup_runs_total {}\n",
        state
            .metrics
            .local_warmup_runs_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_warmup_failures_total Failed local warmup attempts\n");
    body.push_str("# TYPE llm_gateway_local_warmup_failures_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_warmup_failures_total {}\n",
        state
            .metrics
            .local_warmup_failures_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_ollama_timing_samples_total Local Ollama responses with timing payload\n");
    body.push_str("# TYPE llm_gateway_local_ollama_timing_samples_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_ollama_timing_samples_total {}\n",
        state
            .metrics
            .local_ollama_timing_samples_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_ollama_load_ms_total Sum of model load time (ms)\n");
    body.push_str("# TYPE llm_gateway_local_ollama_load_ms_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_ollama_load_ms_total {}\n",
        state
            .metrics
            .local_ollama_load_ms_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_local_ollama_prompt_eval_ms_total Sum of prompt eval time (ms)\n",
    );
    body.push_str("# TYPE llm_gateway_local_ollama_prompt_eval_ms_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_ollama_prompt_eval_ms_total {}\n",
        state
            .metrics
            .local_ollama_prompt_eval_ms_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_local_ollama_eval_ms_total Sum of decode eval time (ms)\n");
    body.push_str("# TYPE llm_gateway_local_ollama_eval_ms_total counter\n");
    body.push_str(&format!(
        "llm_gateway_local_ollama_eval_ms_total {}\n",
        state
            .metrics
            .local_ollama_eval_ms_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_local_adaptive_profile_total Local adaptive tuning profile usage\n",
    );
    body.push_str("# TYPE llm_gateway_local_adaptive_profile_total counter\n");
    let mut adaptive_keys = local_adaptive_profiles.keys().cloned().collect::<Vec<_>>();
    adaptive_keys.sort();
    for profile in adaptive_keys {
        if let Some(count) = local_adaptive_profiles.get(&profile) {
            body.push_str(&format!(
                "llm_gateway_local_adaptive_profile_total{{profile=\"{}\"}} {}\n",
                profile, count
            ));
        }
    }
    body.push_str("# HELP llm_gateway_fuel_supabase_emit_success_total Successful fuel emissions to Supabase\n");
    body.push_str("# TYPE llm_gateway_fuel_supabase_emit_success_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_supabase_emit_success_total {}\n",
        state
            .metrics
            .fuel_supabase_emit_success_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_fuel_supabase_emit_fail_total Failed fuel emissions to Supabase\n",
    );
    body.push_str("# TYPE llm_gateway_fuel_supabase_emit_fail_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_supabase_emit_fail_total {}\n",
        state
            .metrics
            .fuel_supabase_emit_fail_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_fuel_sqlite_fallback_writes_total SQLite fallback writes when Supabase primary path fails\n");
    body.push_str("# TYPE llm_gateway_fuel_sqlite_fallback_writes_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_sqlite_fallback_writes_total {}\n",
        state
            .metrics
            .fuel_sqlite_fallback_writes_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_fuel_settlement_runs_total Settlement runs triggered in gateway\n",
    );
    body.push_str("# TYPE llm_gateway_fuel_settlement_runs_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_settlement_runs_total {}\n",
        state
            .metrics
            .fuel_settlement_runs_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_fuel_settlement_failures_total Settlement runs that failed\n",
    );
    body.push_str("# TYPE llm_gateway_fuel_settlement_failures_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_settlement_failures_total {}\n",
        state
            .metrics
            .fuel_settlement_failures_total
            .load(Ordering::Relaxed)
    ));
    body.push_str(
        "# HELP llm_gateway_fuel_settled_events_total Fuel events updated with settled USD\n",
    );
    body.push_str("# TYPE llm_gateway_fuel_settled_events_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_settled_events_total {}\n",
        state
            .metrics
            .fuel_settled_events_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_fuel_local_energy_updates_total Local energy valuation updates applied\n");
    body.push_str("# TYPE llm_gateway_fuel_local_energy_updates_total counter\n");
    body.push_str(&format!(
        "llm_gateway_fuel_local_energy_updates_total {}\n",
        state
            .metrics
            .fuel_local_energy_updates_total
            .load(Ordering::Relaxed)
    ));
    body.push_str("# HELP llm_gateway_selected_provider_total Selected provider count\n");
    body.push_str("# TYPE llm_gateway_selected_provider_total counter\n");
    for (provider, count) in &provider_counts {
        body.push_str(&format!(
            "llm_gateway_selected_provider_total{{provider=\"{}\"}} {}\n",
            provider, count
        ));
    }
    body.push_str("# HELP llm_gateway_selected_model_total Selected model count\n");
    body.push_str("# TYPE llm_gateway_selected_model_total counter\n");
    for (model, count) in &model_counts {
        body.push_str(&format!(
            "llm_gateway_selected_model_total{{model=\"{}\"}} {}\n",
            model.replace('"', "_"),
            count
        ));
    }
    body.push_str("# HELP llm_gateway_error_by_provider_total Request error count by provider\n");
    body.push_str("# TYPE llm_gateway_error_by_provider_total counter\n");
    for (provider, count) in &error_by_provider {
        body.push_str(&format!(
            "llm_gateway_error_by_provider_total{{provider=\"{}\"}} {}\n",
            provider, count
        ));
    }
    body.push_str("# HELP llm_gateway_error_by_model_total Request error count by model\n");
    body.push_str("# TYPE llm_gateway_error_by_model_total counter\n");
    for (model, count) in &error_by_model {
        body.push_str(&format!(
            "llm_gateway_error_by_model_total{{model=\"{}\"}} {}\n",
            model.replace('"', "_"),
            count
        ));
    }

    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

async fn fuel(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    let by_provider = state.metrics.selected_by_provider.read().await.clone();
    let by_model = state.metrics.selected_by_model.read().await.clone();
    let errors_by_provider = state.metrics.error_by_provider.read().await.clone();
    let errors_by_model = state.metrics.error_by_model.read().await.clone();
    let prompt = state
        .metrics
        .estimated_prompt_tokens_total
        .load(Ordering::Relaxed);
    let completion = state
        .metrics
        .estimated_completion_tokens_total
        .load(Ordering::Relaxed);
    let total = state.metrics.total_requests.load(Ordering::Relaxed);
    let success = state.metrics.success_requests.load(Ordering::Relaxed);
    let failed = state.metrics.error_requests.load(Ordering::Relaxed);

    success_envelope_json(
        request_id,
        json!({
            "calls": {
                "total": total,
                "success": success,
                "failed": failed,
                "stream": state.metrics.stream_requests.load(Ordering::Relaxed),
                "non_stream": state.metrics.non_stream_requests.load(Ordering::Relaxed),
                "by_provider": by_provider,
                "by_model": by_model,
                "errors_by_provider": errors_by_provider,
                "errors_by_model": errors_by_model
            },
            "fuel": {
                "estimated_prompt_tokens_total": prompt,
                "estimated_completion_tokens_total": completion,
                "estimated_tokens_total": prompt + completion,
                "notes": [
                    "Token estimates are heuristic, not provider-billed exact usage",
                    "For stream requests, completion token estimate may be undercounted"
                ]
            },
            "pipeline": {
                "supabase_primary_enabled": state.config.supabase.fuel_primary_enabled,
                "sqlite_fallback_enabled": state.config.supabase.sqlite_fallback_enabled,
                "supabase_emit_success_total": state.metrics.fuel_supabase_emit_success_total.load(Ordering::Relaxed),
                "supabase_emit_fail_total": state.metrics.fuel_supabase_emit_fail_total.load(Ordering::Relaxed),
                "sqlite_fallback_writes_total": state.metrics.fuel_sqlite_fallback_writes_total.load(Ordering::Relaxed),
                "settlement_runs_total": state.metrics.fuel_settlement_runs_total.load(Ordering::Relaxed),
                "settled_events_total": state.metrics.fuel_settled_events_total.load(Ordering::Relaxed)
            },
            "uptime_seconds": state.metrics.started.elapsed().as_secs()
        }),
    )
}

async fn fuel_daily(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let request_id = request_id_from_headers(&headers);
    let days = read_daily_fuel(&state.fuel_db_path, 90).unwrap_or_default();
    success_envelope_json(
        request_id,
        json!({
            "db_path": state.fuel_db_path,
            "days": days
        }),
    )
}

fn sum_numbers_for_keys(value: &serde_json::Value, keys: &[&str]) -> f64 {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| {
                let self_value = if keys.iter().any(|name| name == &k.as_str()) {
                    parse_numeric_json(v)
                } else {
                    0.0
                };
                self_value + sum_numbers_for_keys(v, keys)
            })
            .sum(),
        serde_json::Value::Array(items) => {
            items.iter().map(|v| sum_numbers_for_keys(v, keys)).sum()
        }
        _ => 0.0,
    }
}

fn parse_numeric_json(value: &serde_json::Value) -> f64 {
    match value {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn supabase_credentials(config: &SupabaseConfig) -> anyhow::Result<(&str, &str)> {
    let url = config
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase URL not configured"))?;
    let key = config
        .service_role_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Supabase service_role_key not configured"))?;
    Ok((url, key))
}

async fn get_json_with_retries<F>(
    retries: usize,
    backoff_ms: u64,
    mut build: F,
) -> anyhow::Result<serde_json::Value>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..=retries {
        match build().send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return resp
                        .json::<serde_json::Value>()
                        .await
                        .map_err(|e| anyhow::anyhow!("json parse error: {e}"));
                }
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                last_error = Some(anyhow::anyhow!("http {} {}", status, body));
            }
            Err(e) => {
                last_error = Some(anyhow::anyhow!("transport error: {e}"));
            }
        }
        if attempt < retries {
            let factor = 1_u64 << attempt.min(6);
            tokio::time::sleep(Duration::from_millis(backoff_ms.saturating_mul(factor))).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("request failed")))
}

async fn fetch_fuel_events_for_settlement(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
    limit: u32,
) -> anyhow::Result<Vec<FuelEventForSettlement>> {
    let (url, key) = supabase_credentials(config)?;
    let mut endpoint = reqwest::Url::parse(&format!(
        "{}/rest/v1/fuel_events",
        url.trim_end_matches('/')
    ))?;
    endpoint
        .query_pairs_mut()
        .append_pair("select", "event_id,occurred_at,units,metadata")
        .append_pair("unit_type", "eq.llm_tokens")
        .append_pair("source", "eq.llm-gateway")
        .append_pair("occurred_at", &format!("gte.{}", from.to_rfc3339()))
        .append_pair("occurred_at", &format!("lt.{}", to.to_rfc3339()))
        .append_pair("order", "occurred_at.asc")
        .append_pair("limit", &limit.to_string());

    let rows = client
        .get(endpoint)
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<serde_json::Value>>()
        .await?;

    let mut out = Vec::new();
    for row in rows {
        let Some(event_id) = row
            .get("event_id")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
        else {
            continue;
        };
        let Some(occurred_at_raw) = row.get("occurred_at").and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Ok(occurred_at) = chrono::DateTime::parse_from_rfc3339(occurred_at_raw) else {
            continue;
        };
        let metadata = row
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        let provider = metadata
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_ascii_lowercase();
        if provider != "openai" && provider != "anthropic" {
            continue;
        }
        let model = metadata
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let request_id = metadata
            .get("request_id")
            .or_else(|| metadata.get("trace_id"))
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string);
        out.push(FuelEventForSettlement {
            event_id,
            occurred_at: occurred_at.with_timezone(&chrono::Utc),
            units: row.get("units").map(parse_numeric_json).unwrap_or(0.0),
            provider,
            model,
            request_id,
        });
    }
    Ok(out)
}

async fn openai_settlement_aggregate(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<SettlementAggregate> {
    let key = config
        .settlement_openai_admin_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("OpenAI admin key not configured for settlement"))?;
    let base = config
        .settlement_openai_base_url
        .as_deref()
        .unwrap_or("https://api.openai.com")
        .trim_end_matches('/')
        .to_string();
    let retries = config.settlement_retry_max_retries;
    let backoff = config.settlement_retry_backoff_base_ms;

    let usage = get_json_with_retries(retries, backoff, || {
        client
            .get(format!("{base}/v1/organization/usage/completions"))
            .bearer_auth(key)
            .query(&[
                ("start_time", from.timestamp().to_string()),
                ("end_time", to.timestamp().to_string()),
                ("bucket_width", "1d".to_string()),
                ("group_by[]", "model".to_string()),
            ])
    })
    .await?;

    let costs = get_json_with_retries(retries, backoff, || {
        client
            .get(format!("{base}/v1/organization/costs"))
            .bearer_auth(key)
            .query(&[
                ("start_time", from.timestamp().to_string()),
                ("end_time", to.timestamp().to_string()),
                ("bucket_width", "1d".to_string()),
            ])
    })
    .await?;

    let usage_tokens = sum_numbers_for_keys(
        &usage,
        &[
            "input_tokens",
            "output_tokens",
            "input_cached_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ],
    );
    let settled_usd = sum_numbers_for_keys(
        &costs,
        &["usd", "amount_usd", "cost_usd", "total_cost_usd", "value"],
    );

    Ok(SettlementAggregate {
        usage_tokens,
        settled_usd,
    })
}

async fn anthropic_settlement_aggregate(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<SettlementAggregate> {
    let key = config
        .settlement_anthropic_admin_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Anthropic admin key not configured for settlement"))?;
    let base = config
        .settlement_anthropic_base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com")
        .trim_end_matches('/')
        .to_string();
    let retries = config.settlement_retry_max_retries;
    let backoff = config.settlement_retry_backoff_base_ms;

    let usage = get_json_with_retries(retries, backoff, || {
        client
            .get(format!("{base}/v1/organizations/usage_report/messages"))
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .query(&[
                ("starting_at", from.to_rfc3339()),
                ("ending_at", to.to_rfc3339()),
                ("bucket_width", "1d".to_string()),
            ])
    })
    .await?;

    let costs = get_json_with_retries(retries, backoff, || {
        client
            .get(format!("{base}/v1/organizations/cost_report"))
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .query(&[
                ("starting_at", from.to_rfc3339()),
                ("ending_at", to.to_rfc3339()),
                ("bucket_width", "1d".to_string()),
            ])
    })
    .await?;

    let usage_tokens = sum_numbers_for_keys(
        &usage,
        &[
            "input_tokens",
            "output_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ],
    );
    let settled_usd = sum_numbers_for_keys(
        &costs,
        &["usd", "amount_usd", "cost_usd", "total_cost_usd", "value"],
    );

    Ok(SettlementAggregate {
        usage_tokens,
        settled_usd,
    })
}

async fn apply_settlement_rows_to_supabase(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    settlement_rows: &[serde_json::Value],
    run_id: &str,
) -> anyhow::Result<u64> {
    if settlement_rows.is_empty() {
        return Ok(0);
    }
    let (url, key) = supabase_credentials(config)?;
    let payload = json!({
        "p_rows": settlement_rows,
        "p_run_id": run_id
    });
    let response = client
        .post(format!(
            "{}/rest/v1/rpc/apply_fuel_settlement_rows",
            url.trim_end_matches('/')
        ))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("settlement rpc failed: {} {}", status, body);
    }
    let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(0));
    if let Some(v) = body.as_u64() {
        return Ok(v);
    }
    if let Some(v) = body.get("affected").and_then(serde_json::Value::as_u64) {
        return Ok(v);
    }
    Ok(0)
}

async fn upsert_local_energy_measurement(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    event_id: &str,
    energy_kwh: f64,
    carbon_gco2e: f64,
    confidence: f64,
    metadata: serde_json::Value,
) -> anyhow::Result<bool> {
    let (url, key) = supabase_credentials(config)?;
    let payload = json!({
        "p_event_id": event_id,
        "p_energy_kwh": energy_kwh,
        "p_carbon_gco2e": carbon_gco2e,
        "p_confidence": confidence,
        "p_metadata": metadata
    });
    let response = client
        .post(format!(
            "{}/rest/v1/rpc/upsert_local_energy_measurement",
            url.trim_end_matches('/')
        ))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("local energy rpc failed: {} {}", status, body);
    }
    let body: serde_json::Value = response.json().await.unwrap_or_else(|_| json!(false));
    Ok(body.as_bool().unwrap_or(false))
}

async fn fuel_reconcile_cloud(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CloudReconcileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    validate_gateway_auth(&headers, &state.config.api_key)?;
    if !state.config.supabase.settlement_enabled {
        return Err(error_response_with_request_id(
            StatusCode::BAD_REQUEST,
            "Supabase settlement is disabled by config",
            "invalid_request_error",
            Some(request_id),
        ));
    }

    let now = chrono::Utc::now();
    let to = req
        .to
        .as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or(now);
    let from = req
        .from
        .as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| to - chrono::Duration::hours(24));
    if to <= from {
        return Err(error_response_with_request_id(
            StatusCode::BAD_REQUEST,
            "`to` must be greater than `from`",
            "invalid_request_error",
            Some(request_id),
        ));
    }

    let max_events = req
        .max_events
        .unwrap_or(state.config.supabase.settlement_max_events_per_run)
        .max(1)
        .min(5_000);

    let events = fetch_fuel_events_for_settlement(
        &state.client,
        &state.config.supabase,
        from,
        to,
        max_events,
    )
    .await
    .map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("failed to fetch fuel events: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;

    state
        .metrics
        .fuel_settlement_runs_total
        .fetch_add(1, Ordering::Relaxed);

    let dry_run = req.dry_run.unwrap_or(false);
    let run_id = format!(
        "cloud:{}:{}",
        from.format("%Y%m%d%H%M%S"),
        to.format("%Y%m%d%H%M%S")
    );

    let mut rows = Vec::new();
    let mut provider_summary = serde_json::Map::new();
    let mut grouped: HashMap<(String, chrono::NaiveDate), Vec<FuelEventForSettlement>> =
        HashMap::new();
    for event in events.clone() {
        grouped
            .entry((event.provider.clone(), event.occurred_at.date_naive()))
            .or_default()
            .push(event);
    }

    for ((provider, day), day_events) in grouped {
        let Some(day_start_naive) = day.and_hms_opt(0, 0, 0) else {
            continue;
        };
        let day_start = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            day_start_naive,
            chrono::Utc,
        );
        let day_end = day_start + chrono::Duration::days(1);
        let aggregate = match provider.as_str() {
            "openai" => {
                openai_settlement_aggregate(
                    &state.client,
                    &state.config.supabase,
                    day_start,
                    day_end,
                )
                .await
            }
            "anthropic" => {
                anthropic_settlement_aggregate(
                    &state.client,
                    &state.config.supabase,
                    day_start,
                    day_end,
                )
                .await
            }
            _ => continue,
        };

        match aggregate {
            Ok(agg) => {
                let mut provider_events = 0_u64;
                let mut provider_usd = 0_f64;
                for event in day_events {
                    if agg.usage_tokens <= 0.0 || agg.settled_usd <= 0.0 || event.units <= 0.0 {
                        continue;
                    }
                    let usd_settled = (event.units / agg.usage_tokens) * agg.settled_usd;
                    provider_events += 1;
                    provider_usd += usd_settled;
                    rows.push(json!({
                        "event_id": event.event_id,
                        "usd_settled": usd_settled,
                        "valuation_source": "provider_cost_api",
                        "precision_level": "L2",
                        "confidence": 0.93,
                        "metadata": {
                            "method": "provider_aggregate_proportional_by_tokens",
                            "provider": provider,
                            "model": event.model,
                            "request_id": event.request_id,
                            "window_start": day_start.to_rfc3339(),
                            "window_end": day_end.to_rfc3339()
                        }
                    }));
                }
                provider_summary.insert(
                    format!("{provider}:{day}"),
                    json!({
                        "usage_tokens_total": agg.usage_tokens,
                        "usd_settled_total": agg.settled_usd,
                        "event_rows": provider_events,
                        "allocated_usd": provider_usd
                    }),
                );
            }
            Err(e) => {
                provider_summary.insert(
                    format!("{provider}:{day}"),
                    json!({
                        "error": e.to_string()
                    }),
                );
            }
        }
    }

    let applied = if dry_run {
        0
    } else {
        apply_settlement_rows_to_supabase(&state.client, &state.config.supabase, &rows, &run_id)
            .await
            .map_err(|e| {
                state
                    .metrics
                    .fuel_settlement_failures_total
                    .fetch_add(1, Ordering::Relaxed);
                error_response_with_request_id(
                    StatusCode::BAD_GATEWAY,
                    &format!("settlement apply failed: {e}"),
                    "upstream_error",
                    Some(request_id.clone()),
                )
            })?
    };
    if applied > 0 {
        state
            .metrics
            .fuel_settled_events_total
            .fetch_add(applied, Ordering::Relaxed);
    }

    Ok(success_envelope_json(
        request_id,
        json!({
            "run_id": run_id,
            "from": from.to_rfc3339(),
            "to": to.to_rfc3339(),
            "dry_run": dry_run,
            "events_scanned": events.len(),
            "rows_prepared": rows.len(),
            "rows_applied": applied,
            "provider_summary": provider_summary
        }),
    ))
}

fn release_manifest_path() -> PathBuf {
    std::env::var("LLM_INTENTION_MANIFEST_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("manifest.intentions.json"))
}

fn build_release_manifest(
    config: &Config,
    release_version: &str,
    notes: &[String],
) -> serde_json::Value {
    json!({
        "schema_version": "manifest.intentions.v1",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "service": "llm-gateway",
        "release_version": release_version,
        "contracts": {
            "modes": MODE_CONTRACT_VERSION,
            "code247": CODE247_CONTRACT_VERSION,
            "response_envelope": RESPONSE_ENVELOPE_SCHEMA
        },
        "defaults": {
            "mode": config.default_mode,
            "ci_target": "code247-ci/main",
            "fallback_behavior": "provider-fallback-with-timeout"
        },
        "notes": notes
    })
}

fn write_release_manifest(payload: &serde_json::Value) -> anyhow::Result<PathBuf> {
    let path = release_manifest_path();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&path, serde_json::to_vec_pretty(payload)?)?;
    Ok(path)
}

async fn publish_release_manifest_endpoint(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ReleaseManifestRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    validate_gateway_auth(&headers, &state.config.api_key)?;
    let release_version = req
        .release_version
        .or_else(|| std::env::var("LLM_RELEASE_VERSION").ok())
        .unwrap_or_else(|| "0.3.0".to_string());
    let notes = req.notes.unwrap_or_default();
    let payload = build_release_manifest(&state.config, &release_version, &notes);
    let path = write_release_manifest(&payload).map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("failed to write manifest.intentions.json: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;
    Ok(success_envelope_json(
        request_id,
        json!({
            "written": true,
            "path": path.to_string_lossy(),
            "manifest": payload
        }),
    ))
}

async fn qc_samples(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<QcSamplesQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    validate_gateway_auth(&headers, &state.config.api_key)?;
    let rows = read_qc_samples(&state.fuel_db_path, &q).map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("qc db error: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;
    Ok(success_envelope_json(
        request_id,
        json!({
            "filters": {
                "day": q.day,
                "provider": q.provider,
                "success": q.success,
                "limit": q.limit.unwrap_or(50).clamp(1, 500)
            },
            "count": rows.len(),
            "samples": rows
        }),
    ))
}

async fn onboarding_sync(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<OnboardingSyncRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let jwt = bearer_token(&headers).ok_or_else(|| {
        error_response_with_request_id(
            StatusCode::UNAUTHORIZED,
            "Missing onboarding JWT",
            "invalid_request_error",
            Some(request_id.clone()),
        )
    })?;
    let claims =
        verify_onboarding_jwt(&jwt, &state.config.security).map_err(|(status, json)| {
            error_response_with_request_id(
                status,
                &json.error.message,
                &json.error.error_type,
                Some(request_id.clone()),
            )
        })?;
    let app_name = req
        .app_name
        .or(claims.app_name)
        .unwrap_or(claims.sub)
        .trim()
        .to_string();
    let rotate = req.rotate.unwrap_or(false);
    let (client_id, api_key, created_or_rotated) =
        upsert_api_client_by_app(&state.fuel_db_path, &app_name, rotate).map_err(|e| {
            error_response_with_request_id(
                StatusCode::BAD_GATEWAY,
                &format!("onboarding db error: {e}"),
                "upstream_error",
                Some(request_id.clone()),
            )
        })?;
    Ok(success_envelope_json(
        request_id,
        json!({
            "client_id": client_id,
            "app_name": app_name,
            "api_key": api_key,
            "created_or_rotated": created_or_rotated,
            "jwt": {
                "aud": claims.aud,
                "exp": claims.exp
            }
        }),
    ))
}

async fn admin_daily_client_usage(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ClientUsageQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    validate_gateway_auth(&headers, &state.config.api_key)?;
    let rows = read_daily_client_usage(&state.fuel_db_path, &q).map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("usage db error: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;
    Ok(success_envelope_json(
        request_id,
        json!({
            "filters": {
                "day": q.day,
                "app_name": q.app_name,
                "mode": q.mode,
                "limit": q.limit.unwrap_or(200).clamp(1, 1000)
            },
            "count": rows.len(),
            "rows": rows
        }),
    ))
}

// ============================================================================
// Batch API Handlers (50% cost reduction for non-urgent requests)
// ============================================================================

/// Submit a batch job for async processing
async fn batch_submit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<BatchSubmitRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let client = authenticate_client(&headers, &state).await?;
    enforce_rate_limit(&state, &client).await?;

    let job = create_batch_job(req);
    let job_id = job.id.clone();

    state.batch_queue.enqueue(&job).map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("batch queue error: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;

    // Estimated completion: within 24 hours (batch API guarantee)
    let estimated = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();

    Ok(success_envelope_json(
        request_id,
        json!({
            "job_id": job_id,
            "status": "queued",
            "estimated_completion": estimated,
            "note": "Batch jobs are processed within 24 hours for 50% cost reduction"
        }),
    ))
}

/// Get batch job status and result
async fn batch_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let _client = authenticate_client(&headers, &state).await?;

    let job = state.batch_queue.get_job(&job_id).map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("batch lookup error: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;

    match job {
        Some(job) => Ok(success_envelope_json(
            request_id,
            json!({
                "job_id": job.id,
                "custom_id": job.custom_id,
                "status": job.status,
                "provider": job.provider,
                "model": job.model,
                "created_at": job.created_at,
                "completed_at": job.completed_at,
                "response": job.response,
                "error": job.error,
            }),
        )),
        None => Err(error_response_with_request_id(
            StatusCode::NOT_FOUND,
            "Batch job not found",
            "not_found",
            Some(request_id),
        )),
    }
}

/// Get batch queue statistics
async fn batch_stats(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    validate_gateway_auth(&headers, &state.config.api_key)?;

    let stats = state.batch_queue.stats().map_err(|e| {
        error_response_with_request_id(
            StatusCode::BAD_GATEWAY,
            &format!("batch stats error: {e}"),
            "upstream_error",
            Some(request_id.clone()),
        )
    })?;

    Ok(success_envelope_json(request_id, stats))
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let request_id = request_id_from_headers(&headers);
    let intention_id = header_optional(&headers, "x-intention-id");
    let plan_id = header_optional(&headers, "x-plan-id").or_else(|| intention_id.clone());
    let run_id = header_optional(&headers, "x-run-id");
    let issue_id = header_optional(&headers, "x-issue-id");
    let pr_id = header_optional(&headers, "x-pr-id");
    let deploy_id = header_optional(&headers, "x-deploy-id");
    let ci_target =
        header_optional(&headers, "x-ci-target").unwrap_or_else(|| "code247-ci/main".into());
    let fallback_behavior = header_optional(&headers, "x-fallback-behavior")
        .unwrap_or_else(|| "provider-fallback-with-timeout".into());
    let is_stream = req.stream.unwrap_or(false);
    let client = authenticate_client(&headers, &state).await?;
    enforce_rate_limit(&state, &client).await?;
    let request_mode = parse_mode(req.mode.as_deref(), &state.config.default_mode);
    let request_mode_used = route_mode_name(&request_mode).to_string();
    state.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
    let request_started = Instant::now();
    let qc_should_sample = should_sample_qc(&req, &state.config.qc);
    let qc_key = qc_request_key(&req);
    let mut qc_row: Option<QcSampleRow> = None;
    if is_stream {
        state
            .metrics
            .stream_requests
            .fetch_add(1, Ordering::Relaxed);
    } else {
        state
            .metrics
            .non_stream_requests
            .fetch_add(1, Ordering::Relaxed);
    }
    let mut fuel_delta = FuelDelta {
        calls_total: 1,
        calls_stream: if is_stream { 1 } else { 0 },
        calls_non_stream: if is_stream { 0 } else { 1 },
        ..FuelDelta::default()
    };

    // Track routing decision for Supabase logging
    let used_provider = std::sync::Arc::new(std::sync::RwLock::new(String::from("unknown")));
    let used_model = std::sync::Arc::new(std::sync::RwLock::new(String::from("unknown")));
    let used_mode = std::sync::Arc::new(std::sync::RwLock::new(request_mode_used.clone()));
    let fallback_used_flag = Arc::new(AtomicBool::new(false));

    let result: Result<Response, (StatusCode, Json<ErrorResponse>)> = async {
        if req.messages.is_empty() {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "messages cannot be empty",
                "invalid_request_error",
            ));
        }

        let mode = request_mode.clone();
        let task = classify_task(req.task_hint.as_deref(), &req.messages);
        let budget = execution_budget(&state.config.reliability, &mode);
        let (candidates, mut decision_path, mode_used, task_class) =
            build_route_candidates(&state, &req, &mode, &task).await?;
        let effective_messages = req.messages.clone();
        let stream_requested = is_stream;

        if stream_requested {
            decision_path.push("stream=true".into());
            decision_path.push("premium_plan_skipped_for_stream".into());
            decision_path.push("premium_review_skipped_for_stream".into());
            let prompt_estimate: u64 = effective_messages
                .iter()
                .map(|m| (estimate_tokens_from_text(&m.content) + 4).max(0) as u64)
                .sum();
            fuel_delta.prompt_tokens = prompt_estimate;
            if qc_should_sample && state.config.qc.include_stream {
                let first = candidates.first();
                qc_row = Some(QcSampleRow {
                    sample_key: qc_key.clone(),
                    mode_used: mode_used.clone(),
                    task_class: task_class.clone(),
                    provider: first
                        .map(|c| c.provider.clone())
                        .unwrap_or_else(|| "unknown".into()),
                    model: first
                        .map(|c| c.model.clone())
                        .unwrap_or_else(|| "unknown".into()),
                    is_stream: true,
                    success: true,
                    latency_ms: 0,
                    error_message: None,
                    prompt_excerpt: redact_qc_text(
                        &effective_messages
                            .iter()
                            .map(|m| format!("[{}] {}", m.role, m.content))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        state.config.qc.max_excerpt_chars,
                    ),
                    response_excerpt: "[streamed output not captured]".into(),
                    decision_path: decision_path.join(" > "),
                });
            }
            return stream_chat_completions(
                &state,
                &req,
                &mode,
                &candidates,
                decision_path,
                mode_used,
                task_class,
                effective_messages,
            )
            .await;
        }

        // Request deduplication: check cache for identical recent requests (non-streaming only)
        let cache_key = compute_request_hash(
            &req.messages,
            req.model.as_deref().unwrap_or("auto"),
            &request_mode_used,
        );
        if let Some(cached_response) = cache_lookup(&state, cache_key).await {
            state.metrics.total_requests.fetch_add(1, Ordering::Relaxed);
            decision_path.push("cache=hit".into());
            // Return cached response as JSON
            let mut cached: serde_json::Value = serde_json::from_str(&cached_response)
                .unwrap_or_else(|_| json!({"text": cached_response}));
            if let Some(obj) = cached.as_object_mut() {
                obj.insert(
                    "request_id".into(),
                    serde_json::Value::String(request_id.clone()),
                );
                obj.insert(
                    "output_schema".into(),
                    serde_json::Value::String(
                        "https://logline.world/schemas/llm-gateway.chat-response.v1.schema.json"
                            .into(),
                    ),
                );
            }
            return Ok(Json(cached).into_response());
        }

        let mut output = String::new();
        let mut selected: Option<RouteCandidate> = None;
        let mut last_error_msg = None;
        let started_at = Instant::now();
        let mut attempts = 0usize;
        let mut local_attempts = 0usize;

        for candidate in &candidates {
            if attempts >= budget.max_attempts {
                decision_path.push("budget_reached=max_attempts".into());
                break;
            }
            if started_at.elapsed() >= budget.max_total {
                decision_path.push("budget_reached=max_total_timeout".into());
                break;
            }
            if candidate.provider == "local" && local_attempts >= budget.max_local_attempts {
                decision_path.push(format!(
                    "skip_candidate=local_budget_exhausted::{}",
                    candidate.model
                ));
                continue;
            }

            attempts += 1;
            if candidate.provider == "local" {
                local_attempts += 1;
            }
            decision_path.push(candidate.decision_hint.clone());
            match call_provider_candidate_with_retry(
                &state,
                candidate,
                effective_messages.clone(),
                req.temperature,
                req.max_tokens,
            )
            .await
            {
                Ok(text) => {
                    output = text;
                    selected = Some(candidate.clone());
                    mark_candidate_success(&state, candidate).await;
                    increment_provider_selected(&state, &candidate.provider).await;
                    increment_model_selected(&state, &candidate.model).await;
                    decision_path.push(format!(
                        "selected={}::{}",
                        candidate.provider, candidate.model
                    ));
                    break;
                }
                Err((_, err)) => {
                    fallback_used_flag.store(true, Ordering::Relaxed);
                    state
                        .metrics
                        .fallback_attempt_failures
                        .fetch_add(1, Ordering::Relaxed);
                    mark_candidate_failure(&state, candidate).await;
                    last_error_msg = Some(err.error.message.clone());
                    decision_path.push(format!(
                        "attempt_failed={}::{}",
                        candidate.provider, candidate.model
                    ));
                }
            }
        }

        let selected = selected.ok_or_else(|| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!(
                    "all candidate routes failed{}",
                    last_error_msg
                        .as_ref()
                        .map(|m| format!("; last error: {}", m))
                        .unwrap_or_default()
                ),
                "upstream_error",
            )
        })?;

        let decision = RouteDecision {
            provider: selected.provider.clone(),
            model: selected.model.clone(),
            upstream_url: selected.upstream_url.clone(),
            mode_used,
            task_class,
            decision_path,
            cost_tier: selected.cost_tier.clone(),
        };

        // Capture routing decision for Supabase logging (outside async block)
        if let Ok(mut p) = used_provider.write() {
            *p = decision.provider.clone();
        }
        if let Ok(mut m) = used_model.write() {
            *m = decision.model.clone();
        }
        if let Ok(mut mo) = used_mode.write() {
            *mo = decision.mode_used.clone();
        }

        let usage = estimate_usage(&effective_messages, &output);
        fuel_delta.prompt_tokens = usage.prompt_tokens.max(0) as u64;
        fuel_delta.completion_tokens = usage.completion_tokens.max(0) as u64;
        if usage.prompt_tokens > 0 {
            state
                .metrics
                .estimated_prompt_tokens_total
                .fetch_add(usage.prompt_tokens as u64, Ordering::Relaxed);
        }
        if usage.completion_tokens > 0 {
            state
                .metrics
                .estimated_completion_tokens_total
                .fetch_add(usage.completion_tokens as u64, Ordering::Relaxed);
        }
        if qc_should_sample {
            qc_row = Some(QcSampleRow {
                sample_key: qc_key.clone(),
                mode_used: decision.mode_used.clone(),
                task_class: decision.task_class.clone(),
                provider: decision.provider.clone(),
                model: decision.model.clone(),
                is_stream: false,
                success: true,
                latency_ms: 0,
                error_message: None,
                prompt_excerpt: redact_qc_text(
                    &effective_messages
                        .iter()
                        .map(|m| format!("[{}] {}", m.role, m.content))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    state.config.qc.max_excerpt_chars,
                ),
                response_excerpt: redact_qc_text(&output, state.config.qc.max_excerpt_chars),
                decision_path: decision.decision_path.join(" > "),
            });
        }
        let response = ChatResponse {
            request_id: request_id.clone(),
            output_schema: "https://logline.world/schemas/llm-gateway.chat-response.v1.schema.json",
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".into(),
            created: chrono::Utc::now().timestamp(),
            model: decision.model.clone(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: output,
                },
                finish_reason: "stop".into(),
            }],
            usage,
            lab_meta: LabMeta {
                route: decision.provider,
                upstream_url: maybe_redact_upstream_url(&state.config, &decision.upstream_url),
                model_used: decision.model,
                mode_used: decision.mode_used,
                task_class: decision.task_class,
                decision_path: decision.decision_path,
                cost_tier: decision.cost_tier,
            },
        };

        if is_stream {
            Ok(streaming_chat_response(&response).into_response())
        } else {
            // Store successful response in deduplication cache
            if let Ok(serialized) = serde_json::to_string(&response) {
                cache_store(&state, cache_key, serialized).await;
            }
            Ok(Json(response).into_response())
        }
    }
    .await;

    let latency_ms = request_started.elapsed().as_millis() as u64;
    let provider_used = used_provider
        .read()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "unknown".into());
    let model_used = used_model
        .read()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| "unknown".into());
    let mode_used = used_mode
        .read()
        .ok()
        .map(|g| g.clone())
        .unwrap_or_else(|| request_mode_used.clone());
    let fallback_used = fallback_used_flag.load(Ordering::Relaxed);
    observe_request_latency(
        &state.metrics,
        latency_ms,
        &mode_used,
        &provider_used,
        &model_used,
    );
    match &result {
        Ok(_) => {
            state
                .metrics
                .success_requests
                .fetch_add(1, Ordering::Relaxed);
            fuel_delta.calls_success = 1;
        }
        Err(_) => {
            state.metrics.error_requests.fetch_add(1, Ordering::Relaxed);
            increment_error_breakdown(&state, &provider_used, &model_used).await;
            fuel_delta.calls_failed = 1;
        }
    }
    let success = result.is_ok();
    let error_msg = match &result {
        Err((_, json)) => Some(json.error.message.clone()),
        Ok(_) => None,
    };
    let has_local_timing_signal = state
        .metrics
        .local_ollama_timing_samples_total
        .load(Ordering::Relaxed)
        > 0;
    let local_energy = if provider_used == "local" {
        Some(estimate_local_energy(
            &state.config.local,
            latency_ms,
            has_local_timing_signal,
        ))
    } else {
        None
    };

    let use_supabase_primary =
        state.config.supabase.fuel_primary_enabled && client.is_supabase_billable();
    if !use_supabase_primary {
        if let Err(e) = upsert_daily_fuel(&state.fuel_db_path, fuel_delta) {
            warn!(error = %e, "failed to persist daily fuel");
        } else {
            state
                .metrics
                .fuel_sqlite_fallback_writes_total
                .fetch_add(1, Ordering::Relaxed);
        }
        if let Err(e) =
            upsert_daily_client_usage(&state.fuel_db_path, &client, &request_mode_used, fuel_delta)
        {
            warn!(error = %e, "failed to persist daily client usage");
        }
    }

    // Supabase primary path with fallback to SQLite when enabled.
    if client.is_supabase_billable() && state.config.supabase.fuel_primary_enabled {
        let state_clone = state.clone();
        let client_identity = client.clone();
        let fuel_clone = fuel_delta;
        let provider_for_log = provider_used.clone();
        let model_for_log = model_used.clone();
        let mode_for_log = mode_used.clone();
        let error_for_log = error_msg.clone();
        let request_id_for_log = request_id.clone();
        let plan_id_for_log = plan_id.clone();
        let ci_target_for_log = ci_target.clone();
        let fallback_behavior_for_log = fallback_behavior.clone();
        let intention_id_for_log = intention_id.clone();
        let run_id_for_log = run_id.clone();
        let issue_id_for_log = issue_id.clone();
        let pr_id_for_log = pr_id.clone();
        let deploy_id_for_log = deploy_id.clone();
        tokio::spawn(async move {
            let mut fuel_event_id: Option<String> = None;
            let mut fuel_emit_ok = false;
            let mut emit_last_error: Option<anyhow::Error> = None;
            let metadata = json!({
                "event_type": "llm.request.completed",
                "trace_id": request_id_for_log.clone(),
                "request_id": request_id_for_log.clone(),
                "plan_id": plan_id_for_log.clone(),
                "ci_target": ci_target_for_log.clone(),
                "fallback_behavior": fallback_behavior_for_log.clone(),
                "parent_event_id": serde_json::Value::Null,
                "outcome": if success { "ok" } else { "fail" },
                "provider": provider_for_log.clone(),
                "model": model_for_log.clone(),
                "mode": mode_for_log.clone(),
                "stream": is_stream,
                "latency_ms": latency_ms as u32,
                "prompt_tokens": fuel_clone.prompt_tokens,
                "completion_tokens": fuel_clone.completion_tokens,
                "retry_count": 0,
                "fallback_used": fallback_used,
                "energy_kwh_estimated": local_energy.map(|(kwh, _, _)| kwh),
                "energy_confidence": local_energy.map(|(_, _, conf)| conf),
                "energy_method": if local_energy.is_some() {
                    Some("latency_ms * model_watts / 3_600_000_000")
                } else {
                    None::<&str>
                },
                "intention_id": intention_id_for_log,
                "run_id": run_id_for_log,
                "issue_id": issue_id_for_log,
                "pr_id": pr_id_for_log,
                "deploy_id": deploy_id_for_log,
                "error_message": error_for_log.clone(),
            });

            for attempt in 0..=state_clone.config.supabase.settlement_retry_max_retries {
                match emit_fuel_to_supabase(
                    &state_clone.client,
                    &state_clone.config.supabase,
                    &client_identity,
                    &fuel_clone,
                    metadata.clone(),
                )
                .await
                {
                    Ok(res) => {
                        fuel_emit_ok = true;
                        fuel_event_id = res.event_id;
                        break;
                    }
                    Err(e) => {
                        emit_last_error = Some(e);
                        if attempt < state_clone.config.supabase.settlement_retry_max_retries {
                            let factor = 1_u64 << attempt.min(6);
                            tokio::time::sleep(Duration::from_millis(
                                state_clone
                                    .config
                                    .supabase
                                    .settlement_retry_backoff_base_ms
                                    .saturating_mul(factor),
                            ))
                            .await;
                        }
                    }
                }
            }

            if fuel_emit_ok {
                state_clone
                    .metrics
                    .fuel_supabase_emit_success_total
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                state_clone
                    .metrics
                    .fuel_supabase_emit_fail_total
                    .fetch_add(1, Ordering::Relaxed);
                if let Some(err) = emit_last_error {
                    warn!(error = %err, "failed to emit fuel to Supabase");
                }
                if state_clone.config.supabase.sqlite_fallback_enabled {
                    if let Err(e) = upsert_daily_fuel(&state_clone.fuel_db_path, fuel_clone) {
                        warn!(error = %e, "failed sqlite fallback for daily_fuel");
                    } else {
                        state_clone
                            .metrics
                            .fuel_sqlite_fallback_writes_total
                            .fetch_add(1, Ordering::Relaxed);
                    }
                    if let Err(e) = upsert_daily_client_usage(
                        &state_clone.fuel_db_path,
                        &client_identity,
                        &request_mode_used,
                        fuel_clone,
                    ) {
                        warn!(error = %e, "failed sqlite fallback for daily_client_usage");
                    }
                }
            }

            if let Some((energy_kwh, carbon_gco2e, confidence)) = local_energy {
                if let Some(event_id) = fuel_event_id.as_deref() {
                    if upsert_local_energy_measurement(
                        &state_clone.client,
                        &state_clone.config.supabase,
                        event_id,
                        energy_kwh,
                        carbon_gco2e,
                        confidence,
                        json!({
                            "method": "latency_ms * model_watts / 3_600_000_000",
                            "provider": provider_for_log,
                            "model": model_for_log
                        }),
                    )
                    .await
                    .unwrap_or(false)
                    {
                        state_clone
                            .metrics
                            .fuel_local_energy_updates_total
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            if let Err(e) = log_llm_request_to_supabase(
                &state_clone.client,
                &state_clone.config.supabase,
                &client_identity,
                &request_id_for_log,
                &request_id_for_log,
                plan_id_for_log.as_deref(),
                Some(ci_target_for_log.as_str()),
                Some(fallback_behavior_for_log.as_str()),
                provider_for_log.as_str(),
                model_for_log.as_str(),
                mode_for_log.as_str(),
                fuel_clone.prompt_tokens as u32,
                fuel_clone.completion_tokens as u32,
                latency_ms as u32,
                success,
                fallback_used,
                fuel_event_id.as_deref(),
                error_for_log.as_deref(),
            )
            .await
            {
                warn!(error = %e, "failed to log LLM request to Supabase");
            }
        });
    }

    {
        let obs_event = json!({
            "event_id": uuid::Uuid::new_v4().to_string(),
            "event_type": "llm.request.completed",
            "occurred_at": chrono::Utc::now().to_rfc3339(),
            "source": "llm-gateway",
            "request_id": request_id.clone(),
            "trace_id": request_id.clone(),
            "parent_event_id": serde_json::Value::Null,
            "plan_id": plan_id.clone(),
            "intention_id": intention_id,
            "run_id": run_id,
            "issue_id": issue_id,
            "pr_id": pr_id,
            "deploy_id": deploy_id,
            "payload": {
                "provider": provider_used,
                "model": model_used,
                "mode": mode_used,
                "ci_target": ci_target,
                "fallback_behavior": fallback_behavior,
                "success": success,
                "latency_ms": latency_ms as u32,
                "stream": is_stream,
                "prompt_tokens": fuel_delta.prompt_tokens,
                "completion_tokens": fuel_delta.completion_tokens,
                "fallback_used": fallback_used,
                "energy_kwh_estimated": local_energy.map(|(kwh, _, _)| kwh),
                "energy_confidence": local_energy.map(|(_, _, confidence)| confidence),
                "calling_app": client.calling_app.clone(),
                "error_message": error_msg,
            }
        });

        let state_clone = state.clone();
        tokio::spawn(async move {
            if let Err(e) =
                emit_obs_event(&state_clone.client, &state_clone.config.obs_api, obs_event).await
            {
                warn!(error = %e, "failed to emit llm event to obs-api");
            }
        });
    }

    if qc_should_sample {
        let mut row = qc_row.unwrap_or_else(|| QcSampleRow {
            sample_key: qc_key,
            mode_used: mode_used.clone(),
            task_class: "unknown".into(),
            provider: "unknown".into(),
            model: req.model.clone().unwrap_or_else(|| "unknown".into()),
            is_stream: req.stream.unwrap_or(false),
            success: false,
            latency_ms: 0,
            error_message: None,
            prompt_excerpt: redact_qc_text(
                &req.messages
                    .iter()
                    .map(|m| format!("[{}] {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n"),
                state.config.qc.max_excerpt_chars,
            ),
            response_excerpt: String::new(),
            decision_path: String::new(),
        });
        row.latency_ms = latency_ms;
        if let Err((_, err)) = &result {
            row.success = false;
            row.error_message = Some(redact_qc_text(
                &err.error.message,
                state.config.qc.max_excerpt_chars,
            ));
        }
        if let Err(e) = insert_qc_sample(&state.fuel_db_path, &row, state.config.qc.retention_days)
        {
            warn!(error = %e, "failed to persist qc sample");
        }
    }
    result
}

async fn stream_chat_completions(
    state: &Arc<AppState>,
    req: &ChatRequest,
    mode: &RouteMode,
    candidates: &[RouteCandidate],
    mut decision_path: Vec<String>,
    _mode_used: String,
    _task_class: String,
    messages: Vec<ChatMessage>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let budget = execution_budget(&state.config.reliability, mode);
    let started_at = Instant::now();
    let mut attempts = 0usize;
    let mut local_attempts = 0usize;
    let mut last_error_msg = None;

    for candidate in candidates {
        if attempts >= budget.max_attempts {
            decision_path.push("budget_reached=max_attempts".into());
            break;
        }
        if started_at.elapsed() >= budget.max_total {
            decision_path.push("budget_reached=max_total_timeout".into());
            break;
        }
        if candidate.provider == "local" && local_attempts >= budget.max_local_attempts {
            decision_path.push(format!(
                "skip_candidate=local_budget_exhausted::{}",
                candidate.model
            ));
            continue;
        }

        attempts += 1;
        if candidate.provider == "local" {
            local_attempts += 1;
        }
        decision_path.push(candidate.decision_hint.clone());

        if candidate.provider == "local" {
            let adaptive_profile = resolve_local_adaptive_profile(state).await;
            state
                .metrics
                .observe_local_adaptive_profile(adaptive_profile.as_str())
                .await;
            state
                .metrics
                .local_requests_total
                .fetch_add(1, Ordering::Relaxed);
            let wait_started = Instant::now();
            let queue_wait_timeout =
                local_queue_wait_for_profile(&state.config.local, adaptive_profile);
            let acquire = state.local_inflight.clone().acquire_owned();
            let permit = match tokio::time::timeout(queue_wait_timeout, acquire).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    return Err(error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "local route concurrency limiter unavailable",
                        "routing_error",
                    ));
                }
                Err(_) => {
                    state.metrics.observe_local_queue_timeout();
                    decision_path.push(format!(
                        "attempt_failed={}::{}::queue_timeout",
                        candidate.provider, candidate.model
                    ));
                    last_error_msg = Some("local route queue wait timeout".to_string());
                    continue;
                }
            };
            state
                .metrics
                .observe_local_queue_wait(wait_started.elapsed().as_millis() as u64);
            let local_params = state
                .config
                .local_params_for_route(&candidate.upstream_url, &candidate.model);
            let local_params =
                apply_local_adaptive_profile(&state.config.local, &local_params, adaptive_profile);
            let local_max_tokens =
                local_max_tokens_for_profile(&state.config.local, adaptive_profile, req.max_tokens);
            match call_local_ollama_stream_sse(
                state,
                candidate,
                messages.clone(),
                req.temperature.unwrap_or(0.6),
                local_max_tokens,
                &local_params,
                permit,
            )
            .await
            {
                Ok(resp) => {
                    mark_candidate_success(state, candidate).await;
                    increment_provider_selected(state, &candidate.provider).await;
                    increment_model_selected(state, &candidate.model).await;
                    return Ok(resp);
                }
                Err((_, err)) => {
                    state
                        .metrics
                        .fallback_attempt_failures
                        .fetch_add(1, Ordering::Relaxed);
                    mark_candidate_failure(state, candidate).await;
                    last_error_msg = Some(err.error.message.clone());
                    decision_path.push(format!(
                        "attempt_failed={}::{}",
                        candidate.provider, candidate.model
                    ));
                    continue;
                }
            }
        }

        match call_premium_provider_stream_sse(
            state,
            candidate,
            messages.clone(),
            req.temperature,
            req.max_tokens,
        )
        .await
        {
            Ok(resp) => {
                mark_candidate_success(state, candidate).await;
                increment_provider_selected(state, &candidate.provider).await;
                increment_model_selected(state, &candidate.model).await;
                return Ok(resp);
            }
            Err((_, err)) => {
                state
                    .metrics
                    .fallback_attempt_failures
                    .fetch_add(1, Ordering::Relaxed);
                mark_candidate_failure(state, candidate).await;
                last_error_msg = Some(err.error.message.clone());
                decision_path.push(format!(
                    "attempt_failed={}::{}",
                    candidate.provider, candidate.model
                ));
            }
        }
    }

    Err(error_response(
        StatusCode::BAD_GATEWAY,
        &format!(
            "all candidate routes failed{}",
            last_error_msg
                .as_ref()
                .map(|m| format!("; last error: {m}"))
                .unwrap_or_default()
        ),
        "upstream_error",
    ))
}

async fn call_local_ollama_stream_sse(
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    local_params: &LocalRequestParams,
    permit: OwnedSemaphorePermit,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/api/chat", candidate.upstream_url.trim_end_matches('/'));
    let req = OllamaChatRequest {
        model: candidate.model.clone(),
        messages,
        stream: true,
        keep_alive: local_params.keep_alive.clone(),
        options: OllamaOptions {
            temperature,
            num_predict: max_tokens,
            num_ctx: local_params.options.num_ctx,
            num_batch: local_params.options.num_batch,
            num_thread: local_params.options.num_thread,
            num_gpu: local_params.options.num_gpu,
            top_k: local_params.options.top_k,
            top_p: local_params.options.top_p,
            repeat_penalty: local_params.options.repeat_penalty,
        },
    };

    let response = state
        .client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("local upstream error: {e}"),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("local upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let model = candidate.model.clone();
    let mut upstream = response.bytes_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);
    let state = state.clone();

    tokio::spawn(async move {
        let _permit = permit;
        let mut buf = String::new();
        let mut sent_role = false;

        while let Some(next) = upstream.next().await {
            let bytes = match next {
                Ok(b) => b,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }

                let parsed = match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let piece = parsed["message"]["content"]
                    .as_str()
                    .or_else(|| parsed["response"].as_str())
                    .unwrap_or("");
                if !piece.is_empty() {
                    let delta = if sent_role {
                        json!({"content": piece})
                    } else {
                        sent_role = true;
                        json!({"role": "assistant", "content": piece})
                    };
                    let chunk = json!({
                        "id": id,
                        "object": "chat.completion.chunk",
                        "created": created,
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": delta,
                            "finish_reason": serde_json::Value::Null
                        }]
                    });
                    if tx
                        .send(Ok(Event::default().data(chunk.to_string())))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }

                if parsed["done"].as_bool().unwrap_or(false) {
                    state.metrics.observe_local_ollama_durations(
                        parsed["load_duration"].as_u64(),
                        parsed["prompt_eval_duration"].as_u64(),
                        parsed["eval_duration"].as_u64(),
                    );
                    let final_chunk = json!({
                        "id": id,
                        "object": "chat.completion.chunk",
                        "created": created,
                        "model": model,
                        "choices": [{
                            "index": 0,
                            "delta": {},
                            "finish_reason": "stop"
                        }]
                    });
                    let _ = tx
                        .send(Ok(Event::default().data(final_chunk.to_string())))
                        .await;
                    let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                    return;
                }
            }
        }

        let final_chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let _ = tx
            .send(Ok(Event::default().data(final_chunk.to_string())))
            .await;
        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
    });

    let sse = Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default());
    Ok((StatusCode::OK, sse).into_response())
}

async fn call_premium_provider_stream_sse(
    state: &AppState,
    candidate: &RouteCandidate,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    match candidate.provider.as_str() {
        "openai" => {
            let key = state
                .config
                .premium
                .openai
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "OpenAI API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_openai_stream_sse(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        "anthropic" => {
            let key = state
                .config
                .premium
                .anthropic
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "Anthropic API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_anthropic_stream_sse(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        "gemini" => {
            let key = state
                .config
                .premium
                .gemini
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "Gemini API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_gemini_stream_sse(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        _ => Err(error_response(
            StatusCode::BAD_GATEWAY,
            "No valid provider route",
            "routing_error",
        )),
    }
}

async fn call_openai_stream_sse(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    // OpenAI automatic prompt caching is FREE for prompts ≥1024 tokens
    let body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature.unwrap_or(0.5),
        "max_tokens": max_tokens.unwrap_or(1024),
        "stream": true,
        "store": true,  // Enable prompt caching with extended retention
        "metadata": {"source": "llm-gateway"}
    });

    let response = state
        .client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("openai upstream error: {e}"),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("openai upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let mut upstream = response.bytes_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let mut buf = String::new();
        while let Some(next) = upstream.next().await {
            let bytes = match next {
                Ok(b) => b,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                if let Some(payload) = line.strip_prefix("data:") {
                    let payload = payload.trim();
                    let _ = tx.send(Ok(Event::default().data(payload))).await;
                    if payload == "[DONE]" {
                        return;
                    }
                }
            }
        }
        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
    });

    let sse = Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default());
    Ok((StatusCode::OK, sse).into_response())
}

async fn call_anthropic_stream_sse(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
    let mut system_blocks = Vec::new();
    let mut user_messages = Vec::new();
    for m in messages {
        if m.role == "system" {
            // Use array format with cache_control for prompt caching (90% cost reduction)
            system_blocks.push(json!({
                "type": "text",
                "text": m.content,
                "cache_control": {"type": "ephemeral"}
            }));
        } else {
            user_messages.push(json!({"role": if m.role == "assistant" {"assistant"} else {"user"}, "content": m.content}));
        }
    }
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens.unwrap_or(1024),
        "messages": user_messages,
        "temperature": temperature.unwrap_or(0.5),
        "stream": true
    });
    if !system_blocks.is_empty() {
        // Array format enables prompt caching
        body["system"] = json!(system_blocks);
    }

    let response = state
        .client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31") // Enable prompt caching
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("anthropic upstream error: {e}"),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("anthropic upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let model_name = model.to_string();
    let mut upstream = response.bytes_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let mut buf = String::new();
        let mut current_event = String::new();
        let mut sent_role = false;

        while let Some(next) = upstream.next().await {
            let bytes = match next {
                Ok(b) => b,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }

                if let Some(event_name) = line.strip_prefix("event:") {
                    current_event = event_name.trim().to_string();
                    continue;
                }
                if let Some(data_raw) = line.strip_prefix("data:") {
                    let data_raw = data_raw.trim();
                    if current_event == "message_stop" {
                        let final_chunk = json!({
                            "id": id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_name,
                            "choices": [{
                                "index": 0,
                                "delta": {},
                                "finish_reason": "stop"
                            }]
                        });
                        let _ = tx
                            .send(Ok(Event::default().data(final_chunk.to_string())))
                            .await;
                        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                        return;
                    }

                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_raw) {
                        let delta_text = if current_event == "content_block_delta" {
                            v["delta"]["text"].as_str().unwrap_or("")
                        } else {
                            ""
                        };
                        if !delta_text.is_empty() {
                            let delta = if sent_role {
                                json!({"content": delta_text})
                            } else {
                                sent_role = true;
                                json!({"role":"assistant","content": delta_text})
                            };
                            let chunk = json!({
                                "id": id,
                                "object": "chat.completion.chunk",
                                "created": created,
                                "model": model_name,
                                "choices": [{
                                    "index": 0,
                                    "delta": delta,
                                    "finish_reason": serde_json::Value::Null
                                }]
                            });
                            let _ = tx.send(Ok(Event::default().data(chunk.to_string()))).await;
                        }
                    }
                }
            }
        }

        let final_chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let _ = tx
            .send(Ok(Event::default().data(final_chunk.to_string())))
            .await;
        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
    });

    let sse = Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default());
    Ok((StatusCode::OK, sse).into_response())
}

async fn call_gemini_stream_sse(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let url = format!(
        "{}/v1beta/models/{}:streamGenerateContent?key={}",
        base_url.trim_end_matches('/'),
        model,
        api_key
    );

    // Extract system messages for implicit caching (Gemini caches system_instruction automatically)
    let system_parts: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| json!({"text": m.content}))
        .collect();

    let contents: Vec<serde_json::Value> = messages
        .into_iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            json!({
                "role": if m.role == "assistant" { "model" } else { "user" },
                "parts": [{"text": m.content}]
            })
        })
        .collect();

    // Build request with system_instruction for implicit caching (≥1024 tokens cached automatically)
    let mut body = json!({
        "contents": contents,
        "generationConfig": {
            "temperature": temperature.unwrap_or(0.5),
            "maxOutputTokens": max_tokens.unwrap_or(1024)
        }
    });

    // Add system_instruction if present (enables implicit caching)
    if !system_parts.is_empty() {
        body["system_instruction"] = json!({"parts": system_parts});
    }

    let response = state
        .client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("gemini upstream error: {e}"),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("gemini upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();
    let model_name = model.to_string();
    let mut upstream = response.bytes_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let mut buf = String::new();
        let mut sent_role = false;

        while let Some(next) = upstream.next().await {
            let bytes = match next {
                Ok(b) => b,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }

                let payload = line.strip_prefix("data:").map_or(line.as_str(), str::trim);
                let parsed = match serde_json::from_str::<serde_json::Value>(payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let text = parsed["candidates"][0]["content"]["parts"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|part| part["text"].as_str())
                    .unwrap_or("");
                if text.is_empty() {
                    continue;
                }
                let delta = if sent_role {
                    json!({"content": text})
                } else {
                    sent_role = true;
                    json!({"role": "assistant", "content": text})
                };
                let chunk = json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model_name,
                    "choices": [{
                        "index": 0,
                        "delta": delta,
                        "finish_reason": serde_json::Value::Null
                    }]
                });
                let _ = tx.send(Ok(Event::default().data(chunk.to_string()))).await;
            }
        }

        let final_chunk = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        let _ = tx
            .send(Ok(Event::default().data(final_chunk.to_string())))
            .await;
        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
    });

    let sse = Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default());
    Ok((StatusCode::OK, sse).into_response())
}

fn estimate_tokens_from_text(text: &str) -> i32 {
    let chars = text.chars().count();
    if chars == 0 {
        0
    } else {
        ((chars + 3) / 4) as i32
    }
}

fn estimate_usage(messages: &[ChatMessage], output: &str) -> Usage {
    let prompt_tokens: i32 = messages
        .iter()
        .map(|m| estimate_tokens_from_text(&m.content) + 4)
        .sum();
    let completion_tokens = estimate_tokens_from_text(output);
    Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}

fn streaming_chat_response(resp: &ChatResponse) -> Response {
    let content = resp
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .unwrap_or_default();
    let chunks = chunk_text(content, 120);
    let mut sse_payload = String::new();

    if chunks.is_empty() {
        let chunk = json!({
            "id": resp.id,
            "object": "chat.completion.chunk",
            "created": resp.created,
            "model": resp.model,
            "choices": [{
                "index": 0,
                "delta": {"role": "assistant", "content": ""},
                "finish_reason": serde_json::Value::Null
            }]
        });
        sse_payload.push_str("data: ");
        sse_payload.push_str(&chunk.to_string());
        sse_payload.push_str("\n\n");
    } else {
        for (idx, piece) in chunks.iter().enumerate() {
            let delta = if idx == 0 {
                json!({"role": "assistant", "content": piece})
            } else {
                json!({"content": piece})
            };
            let chunk = json!({
                "id": resp.id,
                "object": "chat.completion.chunk",
                "created": resp.created,
                "model": resp.model,
                "choices": [{
                    "index": 0,
                    "delta": delta,
                    "finish_reason": serde_json::Value::Null
                }]
            });
            sse_payload.push_str("data: ");
            sse_payload.push_str(&chunk.to_string());
            sse_payload.push_str("\n\n");
        }
    }

    let final_chunk = json!({
        "id": resp.id,
        "object": "chat.completion.chunk",
        "created": resp.created,
        "model": resp.model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    });
    sse_payload.push_str("data: ");
    sse_payload.push_str(&final_chunk.to_string());
    sse_payload.push_str("\n\n");
    sse_payload.push_str("data: [DONE]\n\n");

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/event-stream"),
            (header::CACHE_CONTROL, "no-cache"),
            (header::CONNECTION, "keep-alive"),
        ],
        sse_payload,
    )
        .into_response()
}

fn chunk_text(input: &str, chunk_size: usize) -> Vec<String> {
    if input.is_empty() || chunk_size == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        current.push(ch);
        if current.len() >= chunk_size {
            out.push(current);
            current = String::new();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

async fn build_route_candidates(
    state: &AppState,
    req: &ChatRequest,
    mode: &RouteMode,
    task: &TaskClass,
) -> Result<(Vec<RouteCandidate>, Vec<String>, String, String), (StatusCode, Json<ErrorResponse>)> {
    let mut decision_path = Vec::new();
    let mode_used = match mode {
        RouteMode::Genius => "genius",
        RouteMode::Fast => "fast",
        RouteMode::Code => "code",
    }
    .to_string();
    let task_class = task_name(task).to_string();
    let _model_query = req.model.as_deref().unwrap_or("default").to_lowercase();

    let mut candidates = Vec::new();
    match mode {
        RouteMode::Genius => {
            // Best reasoning models (expensive): opus, gpt-5.2, gemini-pro
            decision_path.push("mode=genius".into());
            candidates.extend(genius_candidates(&state.config));
        }
        RouteMode::Fast => {
            // Cheapest premium models: haiku, flash, gpt-5.1-chat
            decision_path.push("mode=fast".into());
            candidates.extend(fast_candidates(&state.config));
        }
        RouteMode::Code => {
            // Local Qwen first, premium code models as fallback
            decision_path.push("mode=code".into());
            candidates.extend(code_candidates(
                &state.config,
                &state.config.routes,
                req.model.as_deref(),
            ));
        }
    }

    let healthy_candidates = filter_candidates_with_circuit_breaker(state, candidates).await;
    if healthy_candidates.is_empty() {
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            "No candidate routes available for request",
            "routing_error",
        ));
    }

    Ok((healthy_candidates, decision_path, mode_used, task_class))
}

fn observe_request_latency(
    metrics: &GatewayMetrics,
    latency_ms: u64,
    mode_used: &str,
    provider_used: &str,
    model_used: &str,
) {
    metrics
        .total_latency_ms
        .fetch_add(latency_ms, Ordering::Relaxed);
    metrics.observe_latency(mode_used, provider_used, model_used, latency_ms);
    loop {
        let current = metrics.max_latency_ms.load(Ordering::Relaxed);
        if latency_ms <= current {
            break;
        }
        if metrics
            .max_latency_ms
            .compare_exchange(current, latency_ms, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            break;
        }
    }
}

async fn increment_provider_selected(state: &AppState, provider: &str) {
    let mut guard = state.metrics.selected_by_provider.write().await;
    let entry = guard.entry(provider.to_string()).or_insert(0);
    *entry += 1;
}

async fn increment_model_selected(state: &AppState, model: &str) {
    let mut guard = state.metrics.selected_by_model.write().await;
    let entry = guard.entry(model.to_string()).or_insert(0);
    *entry += 1;
}

async fn increment_error_breakdown(state: &AppState, provider: &str, model: &str) {
    let mut provider_guard = state.metrics.error_by_provider.write().await;
    let provider_entry = provider_guard.entry(provider.to_string()).or_insert(0);
    *provider_entry += 1;
    drop(provider_guard);

    let mut model_guard = state.metrics.error_by_model.write().await;
    let model_entry = model_guard.entry(model.to_string()).or_insert(0);
    *model_entry += 1;
}

async fn filter_candidates_with_circuit_breaker(
    state: &AppState,
    candidates: Vec<RouteCandidate>,
) -> Vec<RouteCandidate> {
    let now = Instant::now();
    let guard = state.route_health.read().await;
    let filtered: Vec<RouteCandidate> = candidates
        .iter()
        .filter(|c| {
            let key = candidate_key(c);
            match guard.get(&key).and_then(|h| h.open_until) {
                Some(until) => until <= now,
                None => true,
            }
        })
        .cloned()
        .collect();
    if filtered.is_empty() {
        candidates
    } else {
        filtered
    }
}

async fn mark_candidate_success(state: &AppState, candidate: &RouteCandidate) {
    let key = candidate_key(candidate);
    let mut guard = state.route_health.write().await;
    guard.remove(&key);
}

async fn mark_candidate_failure(state: &AppState, candidate: &RouteCandidate) {
    let key = candidate_key(candidate);
    let mut guard = state.route_health.write().await;
    let entry = guard.entry(key).or_default();
    let now = Instant::now();
    let was_open = entry.open_until.map(|u| u > now).unwrap_or(false);
    entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
    if entry.consecutive_failures >= state.config.reliability.circuit_breaker_failure_threshold
        && !was_open
    {
        entry.open_until =
            Some(now + Duration::from_secs(state.config.reliability.circuit_breaker_cooldown_secs));
        state
            .metrics
            .circuit_breaker_opens
            .fetch_add(1, Ordering::Relaxed);
    }
}

async fn call_provider_candidate_with_retry(
    state: &AppState,
    candidate: &RouteCandidate,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let retries = state.config.reliability.retry_max_retries;
    let base = state.config.reliability.retry_backoff_base_ms;
    let mut last_err = None;

    for attempt in 0..=retries {
        let res =
            call_provider_candidate(state, candidate, messages.clone(), temperature, max_tokens)
                .await;
        match res {
            Ok(text) => return Ok(text),
            Err(err) => {
                let retryable = is_retryable_error(&err.1.error.message);
                last_err = Some(err);
                if !retryable || attempt == retries {
                    break;
                }
                let factor = 1u64 << attempt.min(6);
                tokio::time::sleep(Duration::from_millis(base.saturating_mul(factor))).await;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        error_response(
            StatusCode::BAD_GATEWAY,
            "upstream request failed",
            "upstream_error",
        )
    }))
}

async fn call_provider_candidate(
    state: &AppState,
    candidate: &RouteCandidate,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    match candidate.provider.as_str() {
        "local" => {
            let adaptive_profile = resolve_local_adaptive_profile(state).await;
            state
                .metrics
                .observe_local_adaptive_profile(adaptive_profile.as_str())
                .await;
            state
                .metrics
                .local_requests_total
                .fetch_add(1, Ordering::Relaxed);
            let wait_started = Instant::now();
            let queue_wait_timeout =
                local_queue_wait_for_profile(&state.config.local, adaptive_profile);
            let acquire = state.local_inflight.acquire();
            let _permit = match tokio::time::timeout(queue_wait_timeout, acquire).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(_)) => {
                    return Err(error_response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "local route concurrency limiter unavailable",
                        "routing_error",
                    ));
                }
                Err(_) => {
                    state.metrics.observe_local_queue_timeout();
                    return Err(error_response(
                        StatusCode::GATEWAY_TIMEOUT,
                        "local route queue wait timeout",
                        "upstream_error",
                    ));
                }
            };
            state
                .metrics
                .observe_local_queue_wait(wait_started.elapsed().as_millis() as u64);
            let local_params = state
                .config
                .local_params_for_route(&candidate.upstream_url, &candidate.model);
            let local_params =
                apply_local_adaptive_profile(&state.config.local, &local_params, adaptive_profile);
            let local_max_tokens =
                local_max_tokens_for_profile(&state.config.local, adaptive_profile, max_tokens);
            call_local_ollama(
                state,
                &candidate.upstream_url,
                &candidate.model,
                messages,
                temperature.unwrap_or(0.6),
                local_max_tokens,
                &local_params,
            )
            .await
        }
        "openai" => {
            let key = state
                .config
                .premium
                .openai
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "OpenAI API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_openai(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        "anthropic" => {
            let key = state
                .config
                .premium
                .anthropic
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "Anthropic API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_anthropic(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        "gemini" => {
            let key = state
                .config
                .premium
                .gemini
                .resolved_api_key()
                .ok_or_else(|| {
                    error_response(
                        StatusCode::BAD_GATEWAY,
                        "Gemini API key not configured",
                        "provider_auth_error",
                    )
                })?;
            call_gemini(
                state,
                &candidate.upstream_url,
                &key,
                &candidate.model,
                messages,
                temperature,
                max_tokens,
            )
            .await
        }
        _ => Err(error_response(
            StatusCode::BAD_GATEWAY,
            "No valid provider route",
            "routing_error",
        )),
    }
}

fn spawn_local_warmup_worker(state: Arc<AppState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !state.config.local.warmup_enabled {
            info!("Local warmup worker disabled by config");
            return;
        }

        info!("Local warmup worker started");

        loop {
            let adaptive_profile = resolve_local_adaptive_profile(&state).await;
            state
                .metrics
                .observe_local_adaptive_profile(adaptive_profile.as_str())
                .await;
            let (interval, timeout) =
                local_warmup_schedule_for_profile(&state.config.local, adaptive_profile);
            for route in &state.config.routes {
                let permit = match state.local_inflight.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                let params = state
                    .config
                    .local_params_for_route(&route.url, &route.model);
                let params =
                    apply_local_adaptive_profile(&state.config.local, &params, adaptive_profile);
                let payload = OllamaChatRequest {
                    model: route.model.clone(),
                    messages: vec![ChatMessage {
                        role: "user".into(),
                        content: "warmup".into(),
                    }],
                    stream: false,
                    keep_alive: params.keep_alive.clone(),
                    options: OllamaOptions {
                        temperature: 0.0,
                        num_predict: 1,
                        num_ctx: params.options.num_ctx,
                        num_batch: params.options.num_batch,
                        num_thread: params.options.num_thread,
                        num_gpu: params.options.num_gpu,
                        top_k: params.options.top_k,
                        top_p: params.options.top_p,
                        repeat_penalty: params.options.repeat_penalty,
                    },
                };

                let url = format!("{}/api/chat", route.url.trim_end_matches('/'));
                let result = state
                    .client
                    .post(url)
                    .timeout(timeout)
                    .json(&payload)
                    .send()
                    .await;
                match result {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.json::<OllamaChatResponse>().await {
                                Ok(parsed) => {
                                    state.metrics.observe_local_warmup(true);
                                    state.metrics.observe_local_ollama_durations(
                                        parsed.load_duration,
                                        parsed.prompt_eval_duration,
                                        parsed.eval_duration,
                                    );
                                }
                                Err(e) => {
                                    state.metrics.observe_local_warmup(false);
                                    warn!(error = %e, route = %route.name, "local warmup parse failed");
                                }
                            }
                        } else {
                            state.metrics.observe_local_warmup(false);
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            warn!(
                                route = %route.name,
                                status = %status,
                                body = %body,
                                "local warmup request failed"
                            );
                        }
                    }
                    Err(e) => {
                        state.metrics.observe_local_warmup(false);
                        warn!(error = %e, route = %route.name, "local warmup transport failed");
                    }
                }

                drop(permit);
            }

            tokio::time::sleep(interval).await;
        }
    })
}

fn validate_gateway_auth(
    headers: &HeaderMap,
    gateway_key: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let provided_key = auth.strip_prefix("Bearer ").unwrap_or("");
    if provided_key != gateway_key {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid API key",
            "invalid_request_error",
        ));
    }
    Ok(())
}

fn fast_model_matrix(config: &Config) -> Vec<ModelProfile> {
    let mut all: Vec<ModelProfile> = config
        .routes
        .iter()
        .map(|r| score_local_model(&r.name, &r.model))
        .collect();
    all.extend(premium_profiles(config));
    dedupe_profiles(all)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("llm_gateway=info")
        .init();

    let config = Config::load();
    let port = config.port;

    info!("LLM Gateway v0.3.0 on port {}", port);
    info!("Default mode: {}", config.default_mode);
    info!("Local routes: {}", config.routes.len());
    info!(
        "Local concurrency: max_inflight={} max_queue_wait_ms={}",
        config.local.max_inflight_requests, config.local.max_queue_wait_ms
    );
    info!(
        "Local adaptive tuning: enabled={} min_samples={} p95_degraded_ms={} p99_emergency_ms={}",
        config.local.adaptive_tuning_enabled,
        config.local.adaptive_min_samples,
        config.local.adaptive_p95_degraded_ms,
        config.local.adaptive_p99_emergency_ms
    );
    for r in &config.routes {
        info!("  {} -> {} ({})", r.name, r.url, r.model);
    }

    info!(
        "Premium providers: openai={} anthropic={} gemini={}",
        provider_ready(&config.premium.openai),
        provider_ready(&config.premium.anthropic),
        provider_ready(&config.premium.gemini)
    );
    info!(
        "Security: rate_limit_per_minute={} required_service_scope={} legacy_api_key_mode={} legacy_api_key_sunset_at={}",
        config.security.rate_limit_per_minute,
        config
            .supabase
            .required_service_scope
            .as_deref()
            .unwrap_or("<none>"),
        config.security.legacy_api_key_mode,
        config
            .security
            .legacy_api_key_sunset_at
            .as_deref()
            .unwrap_or("<none>")
    );

    let allow_origins: Vec<HeaderValue> = config
        .security
        .cors_allow_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    let cors = if allow_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(allow_origins)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    };
    let fuel_db_path = default_fuel_db_path();
    init_fuel_db(&fuel_db_path)?;

    // Initialize batch queue (shares fuel_db_path for simplicity)
    let batch_queue = BatchQueue::new(&fuel_db_path.to_string_lossy())?;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_millis(
            config.reliability.per_attempt_timeout_ms,
        ))
        .build()?;
    let batch_db_path = fuel_db_path.to_string_lossy().to_string();
    let config = Arc::new(config);

    let state = Arc::new(AppState {
        client: http_client.clone(),
        route_health: RwLock::new(HashMap::new()),
        local_inflight: Arc::new(Semaphore::new(config.local.max_inflight_requests.max(1))),
        metrics: GatewayMetrics::new(),
        fuel_db_path: fuel_db_path.to_string_lossy().to_string(),
        request_cache: RwLock::new(HashMap::new()),
        rate_limits: Mutex::new(HashMap::new()),
        batch_queue,
        config: (*config).clone(),
    });

    // Spawn batch processor worker for async job processing
    let _batch_worker = spawn_batch_worker(batch_db_path, http_client, config);
    info!("Batch processor worker spawned");

    let _local_warmup_worker = spawn_local_warmup_worker(state.clone());

    let auto_publish_manifest = std::env::var("LLM_RELEASE_AUTOPUBLISH")
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if auto_publish_manifest {
        let release_version =
            std::env::var("LLM_RELEASE_VERSION").unwrap_or_else(|_| "0.3.0".to_string());
        let payload = build_release_manifest(
            &state.config,
            &release_version,
            &["auto-published-on-startup".to_string()],
        );
        match write_release_manifest(&payload) {
            Ok(path) => info!(path = %path.to_string_lossy(), "published manifest.intentions.json"),
            Err(e) => warn!(error = %e, "failed to auto-publish manifest.intentions.json"),
        }
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/metrics/summary", get(metrics_summary))
        .route("/v1/modes", get(mode_contract))
        .route("/v1/contracts/code247", get(code247_contract))
        .route("/v1/onboarding/sync", post(onboarding_sync))
        .route("/v1/fuel", get(fuel))
        .route("/v1/fuel/daily", get(fuel_daily))
        .route("/v1/admin/fuel/reconcile/cloud", post(fuel_reconcile_cloud))
        .route(
            "/v1/admin/release/manifest",
            post(publish_release_manifest_endpoint),
        )
        .route("/v1/admin/usage/daily", get(admin_daily_client_usage))
        .route("/v1/qc/samples", get(qc_samples))
        .route("/v1/models", get(list_models))
        .route("/v1/llm/matrix", get(matrix))
        .route("/v1/chat/completions", post(chat_completions))
        // Batch API (50% discount for non-urgent requests)
        .route("/v1/batch", post(batch_submit))
        .route("/v1/batch/:job_id", get(batch_status))
        .route("/v1/admin/batch/stats", get(batch_stats))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ============================================================================
// Request Deduplication Cache
// ============================================================================

/// Cache TTL for request deduplication (5 minutes)
const REQUEST_CACHE_TTL: Duration = Duration::from_secs(300);

/// Compute a hash for request deduplication
fn compute_request_hash(messages: &[ChatMessage], model: &str, mode: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    model.hash(&mut hasher);
    mode.hash(&mut hasher);
    for msg in messages {
        msg.role.to_lowercase().hash(&mut hasher);
        // Normalize: lowercase, collapse whitespace
        msg.content
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .hash(&mut hasher);
    }
    hasher.finish()
}

/// Try to get a cached response for the given request
async fn cache_lookup(state: &AppState, hash: u64) -> Option<String> {
    let cache = state.request_cache.read().await;
    cache.get(&hash).and_then(|(response, timestamp)| {
        if timestamp.elapsed() < REQUEST_CACHE_TTL {
            Some(response.clone())
        } else {
            None
        }
    })
}

/// Store a response in the cache
async fn cache_store(state: &AppState, hash: u64, response: String) {
    let mut cache = state.request_cache.write().await;
    // Prune old entries if cache is getting large (> 1000 entries)
    if cache.len() > 1000 {
        let now = Instant::now();
        cache.retain(|_, (_, ts)| now.duration_since(*ts) < REQUEST_CACHE_TTL);
    }
    cache.insert(hash, (response, Instant::now()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::HeaderValue};

    fn test_routes() -> Vec<LlmRoute> {
        vec![
            LlmRoute {
                name: "coder-route".into(),
                url: "http://127.0.0.1:11434".into(),
                model: "qwen2.5-coder:7b".into(),
                aliases: vec!["coder".into(), "code".into()],
                keep_alive: None,
                options: LocalOllamaOptions::default(),
            },
            LlmRoute {
                name: "default-route".into(),
                url: "http://127.0.0.1:11435".into(),
                model: "qwen2.5:3b".into(),
                aliases: vec!["default".into(), "fast".into()],
                keep_alive: None,
                options: LocalOllamaOptions::default(),
            },
            LlmRoute {
                name: "other-route".into(),
                url: "http://127.0.0.1:11436".into(),
                model: "llama3.2:3b".into(),
                aliases: vec!["llama".into()],
                keep_alive: None,
                options: LocalOllamaOptions::default(),
            },
        ]
    }

    #[test]
    fn parse_mode_defaults_to_code_for_unknown() {
        let mode = parse_mode(Some("something-else"), "code");
        assert!(matches!(mode, RouteMode::Code));
    }

    #[test]
    fn classify_task_detects_coding() {
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "help me debug this rust code".into(),
        }];
        let task = classify_task(None, &messages);
        assert!(matches!(task, TaskClass::Coding));
    }

    #[test]
    fn chunk_text_splits_content() {
        let chunks = chunk_text("abcdefghij", 3);
        assert_eq!(chunks, vec!["abc", "def", "ghi", "j"]);
    }

    #[test]
    fn usage_estimation_returns_positive_totals() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "You are helpful".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Write a short answer".into(),
            },
        ];
        let usage = estimate_usage(&messages, "Sure, here is a concise answer.");
        assert!(usage.prompt_tokens > 0);
        assert!(usage.completion_tokens > 0);
        assert_eq!(
            usage.total_tokens,
            usage.prompt_tokens + usage.completion_tokens
        );
    }

    #[test]
    fn upstream_url_is_redacted_by_default() {
        let cfg = Config {
            port: 3000,
            api_key: "test-key-1234567890".into(),
            default_mode: "code".into(),
            routes: test_routes(),
            premium: PremiumProviders::default(),
            reliability: ReliabilityPolicy::default(),
            security: SecurityPolicy::default(),
            qc: QcPolicy::default(),
            supabase: SupabaseConfig::default(),
            obs_api: ObsApiConfig::default(),
            model_matrix: ModelMatrix::default(),
            local: LocalPolicy::default(),
        };
        let shown = maybe_redact_upstream_url(&cfg, "http://10.0.0.10:11434");
        assert_eq!(shown, "redacted");
    }

    #[test]
    fn streaming_chat_response_contains_done_marker() {
        let resp = ChatResponse {
            request_id: "req-test".into(),
            output_schema: "https://logline.world/schemas/llm-gateway.chat-response.v1.schema.json",
            id: "chatcmpl-test".into(),
            object: "chat.completion".into(),
            created: 1_700_000_000,
            model: "qwen2.5-coder:7b".into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: "assistant".into(),
                    content: "hello world".into(),
                },
                finish_reason: "stop".into(),
            }],
            usage: Usage {
                prompt_tokens: -1,
                completion_tokens: -1,
                total_tokens: -1,
            },
            lab_meta: LabMeta {
                route: "local".into(),
                upstream_url: "http://127.0.0.1:11434".into(),
                model_used: "qwen2.5-coder:7b".into(),
                mode_used: "code".into(),
                task_class: "coding".into(),
                decision_path: vec!["mode=code".into()],
                cost_tier: "zero".into(),
            },
        };

        let response = streaming_chat_response(&resp);
        assert_eq!(response.status(), StatusCode::OK);
    }

    fn sign_hs256_jwt(secret: &str, claims: serde_json::Value) -> String {
        jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("jwt")
    }

    #[test]
    fn service_jwt_requires_matching_scope_when_configured() {
        let token = sign_hs256_jwt(
            "jwt-secret",
            json!({
                "sub": "code247",
                "role": "service",
                "tenant_id": "voulezvous",
                "scope": "code247:intentions:write",
                "exp": 4_102_444_800u64
            }),
        );
        let config = SupabaseConfig {
            jwt_secret: Some("jwt-secret".into()),
            required_service_scope: Some("llm:invoke".into()),
            ..SupabaseConfig::default()
        };

        let identity = try_supabase_jwt(&token, &config);
        assert!(identity.is_none(), "service jwt with wrong scope must fail");
    }

    #[test]
    fn service_jwt_accepts_matching_scope_when_configured() {
        let token = sign_hs256_jwt(
            "jwt-secret",
            json!({
                "sub": "code247",
                "role": "service",
                "tenant_id": "voulezvous",
                "scope": "llm:invoke code247:intentions:write",
                "exp": 4_102_444_800u64
            }),
        );
        let config = SupabaseConfig {
            jwt_secret: Some("jwt-secret".into()),
            required_service_scope: Some("llm:invoke".into()),
            ..SupabaseConfig::default()
        };

        let identity = try_supabase_jwt(&token, &config).expect("identity");
        assert_eq!(identity.tenant_id.as_deref(), Some("voulezvous"));
        assert_eq!(identity.app_id.as_deref(), Some("code247"));
    }

    #[tokio::test]
    async fn build_route_candidates_code_local_first_premium_fallback() {
        let config = Config {
            port: 3000,
            api_key: "test-key-1234567890".into(),
            default_mode: "code".into(),
            routes: test_routes(),
            premium: PremiumProviders {
                openai: ProviderConfig {
                    enabled: true,
                    api_key: Some("sk-test-openai".into()),
                    api_key_env: None,
                    base_url: Some("https://api.openai.com".into()),
                    default_model: Some("gpt-5-mini".into()),
                },
                anthropic: ProviderConfig {
                    enabled: true,
                    api_key: Some("sk-test-anthropic".into()),
                    api_key_env: None,
                    base_url: Some("https://api.anthropic.com".into()),
                    default_model: Some("claude-sonnet-4".into()),
                },
                gemini: ProviderConfig::default(),
            },
            reliability: ReliabilityPolicy::default(),
            security: SecurityPolicy::default(),
            qc: QcPolicy::default(),
            supabase: SupabaseConfig::default(),
            obs_api: ObsApiConfig::default(),
            model_matrix: ModelMatrix::default(),
            local: LocalPolicy::default(),
        };
        let state = AppState {
            client: reqwest::Client::new(),
            route_health: RwLock::new(HashMap::new()),
            local_inflight: Arc::new(Semaphore::new(2)),
            metrics: GatewayMetrics::new(),
            fuel_db_path: "/tmp/llm-gateway-test-fuel.db".into(),
            request_cache: RwLock::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            batch_queue: BatchQueue::new("/tmp/llm-gateway-test-batch.db").unwrap(),
            config,
        };
        let req = ChatRequest {
            model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "write code".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: Some(false),
            mode: Some("code".into()),
            task_hint: None,
        };

        let (candidates, path, _, _) =
            build_route_candidates(&state, &req, &RouteMode::Code, &TaskClass::Coding)
                .await
                .expect("route candidates");

        // Code mode: local first, premium fallback
        assert!(path.iter().any(|p| p == "mode=code"));
        assert!(!candidates.is_empty());
        // First candidate should be local
        assert_eq!(candidates.first().unwrap().provider, "local");
        // Should have premium fallback
        assert!(candidates
            .iter()
            .any(|c| c.provider == "anthropic" || c.provider == "openai"));
    }

    #[tokio::test]
    async fn genius_mode_uses_premium_models() {
        let config = Config {
            port: 3000,
            api_key: "test-key-1234567890".into(),
            default_mode: "genius".into(),
            routes: test_routes(),
            premium: PremiumProviders {
                openai: ProviderConfig {
                    enabled: true,
                    api_key: Some("sk-test-openai".into()),
                    api_key_env: None,
                    base_url: Some("https://api.openai.com".into()),
                    default_model: Some("gpt-5-mini".into()),
                },
                anthropic: ProviderConfig {
                    enabled: true,
                    api_key: Some("sk-test-anthropic".into()),
                    api_key_env: None,
                    base_url: Some("https://api.anthropic.com".into()),
                    default_model: Some("claude-sonnet-4".into()),
                },
                gemini: ProviderConfig::default(),
            },
            reliability: ReliabilityPolicy::default(),
            security: SecurityPolicy::default(),
            qc: QcPolicy::default(),
            supabase: SupabaseConfig::default(),
            obs_api: ObsApiConfig::default(),
            model_matrix: ModelMatrix::default(),
            local: LocalPolicy::default(),
        };
        let state = AppState {
            client: reqwest::Client::new(),
            route_health: RwLock::new(HashMap::new()),
            local_inflight: Arc::new(Semaphore::new(2)),
            metrics: GatewayMetrics::new(),
            fuel_db_path: "/tmp/llm-gateway-test-fuel.db".into(),
            request_cache: RwLock::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            batch_queue: BatchQueue::new("/tmp/llm-gateway-test-batch.db").unwrap(),
            config,
        };
        let req = ChatRequest {
            model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "complex reasoning task".into(),
            }],
            temperature: None,
            max_tokens: None,
            stream: Some(false),
            mode: Some("genius".into()),
            task_hint: None,
        };

        let (candidates, path, _, _) =
            build_route_candidates(&state, &req, &RouteMode::Genius, &TaskClass::General)
                .await
                .expect("route candidates");

        // Genius mode: premium only
        assert!(path.iter().any(|p| p == "mode=genius"));
        assert!(candidates.iter().all(|c| c.provider != "local"));
    }

    async fn spawn_mock_ollama(delay_divisor: u64) -> String {
        let app = Router::new().route(
            "/api/chat",
            post(move |Json(req): Json<serde_json::Value>| async move {
                let num_predict = req["options"]["num_predict"].as_u64().unwrap_or(0);
                let delay = (num_predict / delay_divisor).min(180);
                tokio::time::sleep(Duration::from_millis(delay)).await;
                Json(json!({
                    "message": {"content": "ok"},
                    "load_duration": 3_000_000,
                    "prompt_eval_duration": 4_000_000,
                    "eval_duration": 5_000_000
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock ollama");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    async fn spawn_mock_openai() -> String {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async move {
                Json(json!({
                    "choices": [{
                        "message": {
                            "content": "premium fallback ok"
                        }
                    }]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock openai");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    fn build_state_for_agent_b(
        local_url: String,
        openai_url: Option<String>,
        local_default_max_tokens: u32,
        queue_wait_ms: u64,
        inflight: usize,
    ) -> Arc<AppState> {
        let mut local = LocalPolicy::default();
        local.default_max_tokens = local_default_max_tokens;
        local.adaptive_degraded_max_tokens = local_default_max_tokens.min(512);
        local.adaptive_emergency_max_tokens = local_default_max_tokens.min(384);
        local.max_queue_wait_ms = queue_wait_ms;
        local.adaptive_tuning_enabled = false;
        local.warmup_enabled = false;

        let premium = if let Some(base_url) = openai_url {
            PremiumProviders {
                openai: ProviderConfig {
                    enabled: true,
                    api_key: Some("sk-test-openai".into()),
                    api_key_env: None,
                    base_url: Some(base_url),
                    default_model: Some("gpt-5.1-chat-latest".into()),
                },
                anthropic: ProviderConfig::default(),
                gemini: ProviderConfig::default(),
            }
        } else {
            PremiumProviders::default()
        };

        let config = Config {
            port: 3000,
            api_key: "test-key-1234567890".into(),
            default_mode: "code".into(),
            routes: vec![LlmRoute {
                name: "local".into(),
                url: local_url,
                model: "qwen2.5-coder:7b".into(),
                aliases: vec!["code".into()],
                keep_alive: None,
                options: LocalOllamaOptions::default(),
            }],
            premium,
            reliability: ReliabilityPolicy::default(),
            security: SecurityPolicy::default(),
            qc: QcPolicy::default(),
            supabase: SupabaseConfig::default(),
            obs_api: ObsApiConfig::default(),
            model_matrix: ModelMatrix::default(),
            local,
        };

        Arc::new(AppState {
            client: reqwest::Client::new(),
            route_health: RwLock::new(HashMap::new()),
            local_inflight: Arc::new(Semaphore::new(inflight.max(1))),
            metrics: GatewayMetrics::new(),
            fuel_db_path: format!("/tmp/llm-gateway-agent-b-{}.db", Uuid::new_v4()),
            request_cache: RwLock::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            batch_queue: BatchQueue::new(&format!(
                "/tmp/llm-gateway-agent-b-batch-{}.db",
                Uuid::new_v4()
            ))
            .unwrap(),
            config,
        })
    }

    async fn invoke_chat(state: Arc<AppState>, prompt: String) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test-key-1234567890"),
        );
        let req = ChatRequest {
            model: None,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: prompt,
            }],
            temperature: Some(0.2),
            max_tokens: None,
            stream: Some(false),
            mode: Some("code".into()),
            task_hint: Some("coding".into()),
        };
        chat_completions(State(state), headers, Json(req))
            .await
            .expect("chat response")
    }

    #[tokio::test]
    async fn agent_b_load_short_burst_improves_p95_p99_with_local_token_caps() {
        let local_url = spawn_mock_ollama(12).await;

        let state_before = build_state_for_agent_b(local_url.clone(), None, 1024, 1200, 2);
        for i in 0..24 {
            let resp = invoke_chat(state_before.clone(), format!("before burst request {i}")).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let _ = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        }
        let before = state_before
            .metrics
            .latency_quantiles_snapshot()
            .by_provider
            .get("local")
            .cloned()
            .expect("before local quantiles");

        let state_after = build_state_for_agent_b(local_url, None, 512, 1200, 2);
        for i in 0..24 {
            let resp = invoke_chat(state_after.clone(), format!("after burst request {i}")).await;
            assert_eq!(resp.status(), StatusCode::OK);
            let _ = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        }
        let after = state_after
            .metrics
            .latency_quantiles_snapshot()
            .by_provider
            .get("local")
            .cloned()
            .expect("after local quantiles");

        println!(
            "agent_b_latency_before_after local p95/p99: before={}/{} after={}/{}",
            before.p95_ms, before.p99_ms, after.p95_ms, after.p99_ms
        );
        assert!(after.p95_ms < before.p95_ms);
        assert!(after.p99_ms < before.p99_ms);
    }

    #[tokio::test]
    async fn agent_b_fallback_local_timeout_to_openai_succeeds() {
        let local_url = spawn_mock_ollama(1).await;
        let openai_url = spawn_mock_openai().await;
        let state = build_state_for_agent_b(local_url, Some(openai_url), 768, 60, 1);

        // Saturate local semaphore so local candidate times out and fallback kicks in.
        let _held = state.local_inflight.clone().acquire_owned().await.unwrap();
        let resp = invoke_chat(state.clone(), "fallback please".into()).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = to_bytes(resp.into_body(), usize::MAX).await.unwrap();

        let fallback_failures = state
            .metrics
            .fallback_attempt_failures
            .load(Ordering::Relaxed);
        let selected = state.metrics.selected_by_provider.read().await;
        let openai_selected = selected.get("openai").copied().unwrap_or(0);
        assert!(fallback_failures >= 1);
        assert!(openai_selected >= 1);
    }

    #[tokio::test]
    async fn agent_b_timeout_when_local_queue_wait_exceeded() {
        let local_url = spawn_mock_ollama(1).await;
        let state = build_state_for_agent_b(local_url, None, 768, 40, 1);
        let _held = state.local_inflight.clone().acquire_owned().await.unwrap();

        let candidate = RouteCandidate {
            provider: "local".into(),
            model: "qwen2.5-coder:7b".into(),
            upstream_url: state.config.routes[0].url.clone(),
            cost_tier: "zero".into(),
            decision_hint: "test".into(),
        };
        let res = call_provider_candidate(
            &state,
            &candidate,
            vec![ChatMessage {
                role: "user".into(),
                content: "timeout case".into(),
            }],
            Some(0.2),
            None,
        )
        .await;

        let (status, err) = res.expect_err("local timeout expected");
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert!(err.error.message.contains("queue wait timeout"));
    }
}
