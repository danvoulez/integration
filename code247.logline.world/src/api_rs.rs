use std::{
    collections::HashSet,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::Sha256;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    adapters_rs::{LinearAdapter, LinearOAuthClient},
    config_rs::Config,
    persistence_rs::{
        IntentionLinkRepository, JobsRepository, LinearOAuthTokenRepository,
        LinearWebhookDeliveryRepository, ManifestIngestionRepository, OAuthStateRepository,
        RunTimelineRepository,
    },
    transition_guard_rs::{
        build_transition_block_message, classify_linear_workflow_state, evaluate_transition,
        workflow_state_label, LinearWorkflowState,
    },
};

#[derive(Clone)]
struct AppState {
    jobs: Arc<Mutex<JobsRepository>>,
    run_timeline: Arc<Mutex<RunTimelineRepository>>,
    oauth_state_store: Arc<Mutex<OAuthStateRepository>>,
    oauth_token_store: Arc<Mutex<LinearOAuthTokenRepository>>,
    manifest_ingestion_store: Arc<Mutex<ManifestIngestionRepository>>,
    intention_link_store: Arc<Mutex<IntentionLinkRepository>>,
    oauth_client: Option<LinearOAuthClient>,
    oauth_state_ttl_seconds: i64,
    linear_team_id: String,
    linear_claim_state_name: String,
    linear_claim_in_progress_state_name: String,
    linear_done_state_type: String,
    linear_ready_for_release_state_name: String,
    linear_api_key: Option<String>,
    linear_api_base_url: String,
    intentions_token: Option<String>,
    auth_allow_legacy_token: bool,
    supabase_jwt_secret: Option<String>,
    supabase_jwt_secret_legacy: Option<String>,
    supabase_jwt_audience: Option<String>,
    scope_jobs_read: String,
    scope_jobs_write: String,
    scope_intentions_write: String,
    scope_intentions_sync: String,
    scope_intentions_read: String,
    scope_admin: String,
    linear_webhook_secret: Option<String>,
    linear_webhook_max_skew_seconds: i64,
    linear_meta_path: String,
    public_url: String,
    obs_api_base_url: Option<String>,
    obs_api_token: Option<String>,
    obs_api_client: Client,
    webhook_delivery_store: Arc<Mutex<LinearWebhookDeliveryRepository>>,
}

const RESPONSE_ENVELOPE_SCHEMA: &str =
    "https://logline.world/schemas/response-envelope.v1.schema.json";
const ERROR_ENVELOPE_SCHEMA: &str = "https://logline.world/schemas/error-envelope.v1.schema.json";

#[derive(Deserialize)]
struct CreateJobInput {
    issue_id: String,
    payload: String,
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntentionIntakeRequest {
    manifest: IntentionManifest,
    source: String,
    revision: Option<String>,
    ci_target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntentionManifest {
    workspace: String,
    project: String,
    updated_at: String,
    intentions: Vec<IntentionRecord>,
}

#[derive(Debug, Deserialize)]
struct IntentionRecord {
    id: String,
    title: String,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    tasks: Vec<IntentionTaskRecord>,
}

#[derive(Debug, Deserialize)]
struct IntentionTaskRecord {
    description: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    due: Option<String>,
    #[serde(default)]
    gate: Option<String>,
}

#[derive(Debug, Serialize)]
struct IntentionIntakeResponse {
    request_id: String,
    deduped: bool,
    linear: IntentionLinearResponse,
    ci: IntentionCiResponse,
}

#[derive(Debug, Serialize)]
struct IntentionLinearResponse {
    intentions: Vec<IntentionLinearLink>,
}

#[derive(Debug, Serialize)]
struct IntentionLinearLink {
    id: String,
    issue_id: String,
    board: String,
}

#[derive(Debug, Serialize)]
struct IntentionCiResponse {
    jobs: Vec<String>,
    queue_id: String,
}

#[derive(Debug, Serialize)]
struct IntentionLinksSnapshotResponse {
    request_id: String,
    workspace: String,
    project: String,
    ingestion: Value,
    links: Vec<IntentionLinearLink>,
}

#[derive(Debug, Deserialize)]
struct IntentionSyncRequest {
    workspace: String,
    project: String,
    results: Vec<IntentionSyncResultInput>,
}

#[derive(Debug, Deserialize)]
struct IntentionSyncResultInput {
    intention_id: String,
    status: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    ci: Option<IntentionSyncCiInput>,
    #[serde(default)]
    evidence: Vec<IntentionSyncEvidenceInput>,
    #[serde(default)]
    set_done_on_success: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct IntentionSyncCiInput {
    #[serde(default)]
    queue_id: Option<String>,
    #[serde(default)]
    job: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntentionSyncEvidenceInput {
    label: String,
    url: String,
}

#[derive(Debug, Serialize)]
struct IntentionSyncResponse {
    request_id: String,
    workspace: String,
    project: String,
    synced: Vec<IntentionSyncResultOutput>,
    errors: Vec<IntentionSyncErrorOutput>,
}

#[derive(Debug, Serialize)]
struct IntentionSyncResultOutput {
    intention_id: String,
    issue_id: String,
    comment_posted: bool,
    moved_to_ready_for_release: bool,
    moved_to_done: bool,
    target_state: Option<String>,
}

#[derive(Debug, Serialize)]
struct IntentionSyncErrorOutput {
    intention_id: String,
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct LinearWebhookAck {
    request_id: String,
    accepted: bool,
    deduped: bool,
    delivery_id: String,
    status: String,
    issue_id: Option<String>,
}

pub async fn serve(
    config: Config,
    jobs: Arc<Mutex<JobsRepository>>,
    run_timeline: Arc<Mutex<RunTimelineRepository>>,
    oauth_state_store: Arc<Mutex<OAuthStateRepository>>,
    oauth_token_store: Arc<Mutex<LinearOAuthTokenRepository>>,
    manifest_ingestion_store: Arc<Mutex<ManifestIngestionRepository>>,
    intention_link_store: Arc<Mutex<IntentionLinkRepository>>,
    webhook_delivery_store: Arc<Mutex<LinearWebhookDeliveryRepository>>,
    oauth_client: Option<LinearOAuthClient>,
) -> Result<()> {
    let app_state = AppState {
        jobs,
        run_timeline,
        oauth_state_store,
        oauth_token_store,
        manifest_ingestion_store,
        intention_link_store,
        oauth_client,
        oauth_state_ttl_seconds: config.linear_oauth_state_ttl_seconds,
        linear_team_id: config.linear_team_id.clone(),
        linear_claim_state_name: config.linear_claim_state_name.clone(),
        linear_claim_in_progress_state_name: config.linear_claim_in_progress_state_name.clone(),
        linear_done_state_type: config.linear_done_state_type.clone(),
        linear_ready_for_release_state_name: config.linear_ready_for_release_state_name.clone(),
        linear_api_key: config.linear_api_key.clone(),
        linear_api_base_url: config.linear_api_base_url.clone(),
        intentions_token: config.code247_intentions_token.clone(),
        auth_allow_legacy_token: config.code247_auth_allow_legacy_token,
        supabase_jwt_secret: config.supabase_jwt_secret.clone(),
        supabase_jwt_secret_legacy: config.supabase_jwt_secret_legacy.clone(),
        supabase_jwt_audience: config.supabase_jwt_audience.clone(),
        scope_jobs_read: config.code247_scope_jobs_read.clone(),
        scope_jobs_write: config.code247_scope_jobs_write.clone(),
        scope_intentions_write: config.code247_scope_intentions_write.clone(),
        scope_intentions_sync: config.code247_scope_intentions_sync.clone(),
        scope_intentions_read: config.code247_scope_intentions_read.clone(),
        scope_admin: config.code247_scope_admin.clone(),
        linear_webhook_secret: config.linear_webhook_secret.clone(),
        linear_webhook_max_skew_seconds: config.linear_webhook_max_skew_seconds,
        linear_meta_path: config.code247_linear_meta_path.clone(),
        public_url: config.code247_public_url.clone(),
        obs_api_base_url: config.obs_api_base_url.clone(),
        obs_api_token: config.obs_api_token.clone(),
        obs_api_client: Client::new(),
        webhook_delivery_store,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/jobs", get(list_jobs).post(create_job))
        .route("/jobs/:job_id/timeline", get(get_job_timeline))
        .route("/oauth/start", get(oauth_start))
        .route("/oauth/callback", get(oauth_callback))
        .route("/oauth/status", get(oauth_status))
        .route("/intentions", post(post_intentions))
        .route("/intentions/sync", post(post_intentions_sync))
        .route("/webhooks/linear", post(post_linear_webhook))
        .route(
            "/intentions/:workspace/:project",
            get(get_intentions_snapshot),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", config.health_port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    success_envelope(
        StatusCode::OK,
        &request_id,
        json!({"status": "ok", "engine": "rust"}),
    )
}

async fn list_jobs(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = ensure_admin_auth(&state, &headers, &request_id, &state.scope_jobs_read)
    {
        return response;
    }
    let jobs = state.jobs.lock().expect("jobs lock").list_recent();
    success_envelope(StatusCode::OK, &request_id, json!({ "jobs": jobs }))
}

async fn create_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateJobInput>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) =
        ensure_admin_auth(&state, &headers, &request_id, &state.scope_jobs_write)
    {
        return response;
    }
    let created = state
        .jobs
        .lock()
        .expect("jobs lock")
        .create_job(&input.issue_id, &input.payload);

    match created {
        Ok(job) => success_envelope(
            StatusCode::CREATED,
            &request_id,
            json!({"job_id": job.id, "status": job.status.as_str()}),
        ),
        Err(err) => error_envelope(
            StatusCode::INTERNAL_SERVER_ERROR,
            &request_id,
            "PERSISTENCE_ERROR",
            "falha ao criar job",
            Some(json!({ "error": err.to_string() })),
        ),
    }
}

async fn get_job_timeline(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = ensure_admin_auth(&state, &headers, &request_id, &state.scope_jobs_read)
    {
        return response;
    }

    let entries = state
        .run_timeline
        .lock()
        .expect("run_timeline lock")
        .list_for_job(&job_id);
    success_envelope(
        StatusCode::OK,
        &request_id,
        json!({
            "job_id": job_id,
            "timeline": entries,
        }),
    )
}

async fn oauth_start(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = ensure_admin_auth(&state, &headers, &request_id, &state.scope_admin) {
        return response;
    }
    let Some(oauth_client) = state.oauth_client.as_ref() else {
        return error_envelope(
            StatusCode::NOT_IMPLEMENTED,
            &request_id,
            "NOT_IMPLEMENTED",
            "linear oauth não configurado",
            None,
        );
    };

    let state_nonce = {
        let store = state
            .oauth_state_store
            .lock()
            .expect("oauth_state_store lock");
        match store.create_state(state.oauth_state_ttl_seconds) {
            Ok(value) => value,
            Err(err) => {
                error!(error=%err, "failed to create oauth state");
                return error_envelope(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &request_id,
                    "INTERNAL_ERROR",
                    "falha ao gerar state oauth",
                    Some(json!({ "error": err.to_string() })),
                );
            }
        }
    };

    let url = oauth_client.authorize_url(&state_nonce);
    Redirect::temporary(&url).into_response()
}

async fn oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    let Some(oauth_client) = state.oauth_client.as_ref() else {
        return error_envelope(
            StatusCode::NOT_IMPLEMENTED,
            &request_id,
            "NOT_IMPLEMENTED",
            "linear oauth não configurado",
            None,
        );
    };

    if let Some(error_code) = query.error {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "OAUTH_CALLBACK_ERROR",
            query
                .error_description
                .as_deref()
                .unwrap_or("oauth callback retornou erro"),
            Some(json!({ "error": error_code })),
        );
    }

    let Some(state_value) = query.state.as_deref() else {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "state ausente no callback",
            None,
        );
    };
    let Some(code_value) = query.code.as_deref() else {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "code ausente no callback",
            None,
        );
    };

    let state_is_valid = {
        let store = state
            .oauth_state_store
            .lock()
            .expect("oauth_state_store lock");
        match store.consume_state(state_value) {
            Ok(valid) => valid,
            Err(err) => {
                error!(error=%err, "failed to validate oauth state");
                return error_envelope(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &request_id,
                    "INTERNAL_ERROR",
                    "falha ao validar state oauth",
                    Some(json!({ "error": err.to_string() })),
                );
            }
        }
    };
    if !state_is_valid {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "state inválido ou expirado",
            None,
        );
    }

    let token = match oauth_client.exchange_code(code_value).await {
        Ok(value) => value,
        Err(err) => {
            error!(error=%err, "failed to exchange oauth code");
            return error_envelope(
                StatusCode::BAD_GATEWAY,
                &request_id,
                "UPSTREAM_ERROR",
                "falha ao trocar code por token",
                Some(json!({ "error": err.to_string() })),
            );
        }
    };

    let refresh_token = token.refresh_token.clone().unwrap_or_default();
    if refresh_token.is_empty() {
        return error_envelope(
            StatusCode::BAD_GATEWAY,
            &request_id,
            "UPSTREAM_ERROR",
            "Linear não retornou refresh_token",
            None,
        );
    }

    let expires_at = (Utc::now() + Duration::seconds(token.expires_in.max(60))).to_rfc3339();
    let upsert_result = {
        let store = state
            .oauth_token_store
            .lock()
            .expect("oauth_token_store lock");
        store.upsert_token(
            &token.access_token,
            &refresh_token,
            &token.token_type,
            token.scope.as_deref(),
            &expires_at,
        )
    };

    if let Err(err) = upsert_result {
        error!(error=%err, "failed to save oauth token");
        return error_envelope(
            StatusCode::INTERNAL_SERVER_ERROR,
            &request_id,
            "PERSISTENCE_ERROR",
            "falha ao persistir token oauth",
            Some(json!({ "error": err.to_string() })),
        );
    }

    let cleanup_result = {
        let store = state
            .oauth_state_store
            .lock()
            .expect("oauth_state_store lock");
        store.cleanup_expired()
    };
    if let Err(err) = cleanup_result {
        error!(error=%err, "failed to cleanup oauth states");
    }

    info!("linear oauth connected");
    success_envelope(
        StatusCode::OK,
        &request_id,
        json!({
            "connected": true,
            "expires_at": expires_at,
            "scope": token.scope,
        }),
    )
}

async fn oauth_status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = ensure_admin_auth(&state, &headers, &request_id, &state.scope_admin) {
        return response;
    }
    if state.oauth_client.is_none() {
        return success_envelope(
            StatusCode::OK,
            &request_id,
            json!({
                "configured": false,
                "connected": false,
            }),
        );
    }

    let token = {
        let store = state
            .oauth_token_store
            .lock()
            .expect("oauth_token_store lock");
        store.get_token()
    };

    match token {
        Some(value) => success_envelope(
            StatusCode::OK,
            &request_id,
            json!({
                "configured": true,
                "connected": true,
                "token_type": value.token_type,
                "scope": value.scope,
                "expires_at": value.expires_at,
            }),
        ),
        None => success_envelope(
            StatusCode::OK,
            &request_id,
            json!({
                "configured": true,
                "connected": false,
            }),
        ),
    }
}

async fn post_linear_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    let Some(secret) = state
        .linear_webhook_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return error_envelope(
            StatusCode::SERVICE_UNAVAILABLE,
            &request_id,
            "CONFIG_ERROR",
            "LINEAR_WEBHOOK_SECRET ausente no servidor",
            None,
        );
    };

    let Some(delivery_id) = header_value(&headers, "Linear-Delivery") else {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "header Linear-Delivery é obrigatório",
            None,
        );
    };
    let Some(signature) = header_value(&headers, "Linear-Signature") else {
        return error_envelope(
            StatusCode::UNAUTHORIZED,
            &request_id,
            "UNAUTHORIZED",
            "header Linear-Signature é obrigatório",
            None,
        );
    };
    let linear_event = header_value(&headers, "Linear-Event");

    if !verify_linear_signature(secret, &body, &signature) {
        return error_envelope(
            StatusCode::UNAUTHORIZED,
            &request_id,
            "UNAUTHORIZED",
            "assinatura de webhook inválida",
            None,
        );
    }

    let payload: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => {
            return error_envelope(
                StatusCode::BAD_REQUEST,
                &request_id,
                "VALIDATION_ERROR",
                "payload webhook inválido (JSON)",
                Some(json!({ "reason": err.to_string() })),
            );
        }
    };

    let Some(timestamp_ms) = extract_webhook_timestamp_ms(&payload) else {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "webhookTimestamp ausente ou inválido no payload",
            None,
        );
    };
    let now_ms = Utc::now().timestamp_millis();
    let max_skew_ms = state.linear_webhook_max_skew_seconds.max(5) * 1000;
    let skew = (now_ms - timestamp_ms).abs();
    if skew > max_skew_ms {
        return error_envelope(
            StatusCode::UNAUTHORIZED,
            &request_id,
            "UNAUTHORIZED",
            "webhookTimestamp fora da janela de segurança",
            Some(json!({
                "skew_ms": skew,
                "max_skew_ms": max_skew_ms,
            })),
        );
    }

    let issue_id = extract_webhook_issue_id(&payload);
    let raw_payload = match String::from_utf8(body.to_vec()) {
        Ok(value) => value,
        Err(err) => {
            return error_envelope(
                StatusCode::BAD_REQUEST,
                &request_id,
                "VALIDATION_ERROR",
                "payload webhook não é UTF-8 válido",
                Some(json!({ "reason": err.to_string() })),
            );
        }
    };

    let enqueued = {
        let store = state
            .webhook_delivery_store
            .lock()
            .expect("webhook_delivery_store lock");
        match store.enqueue(
            &delivery_id,
            linear_event.as_deref(),
            issue_id.as_deref(),
            &raw_payload,
            Some(&signature),
        ) {
            Ok(value) => value,
            Err(err) => {
                return error_envelope(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &request_id,
                    "PERSISTENCE_ERROR",
                    "falha ao enfileirar webhook do Linear",
                    Some(json!({ "error": err.to_string() })),
                );
            }
        }
    };

    let status = if enqueued { "QUEUED" } else { "DEDUPED" };
    emit_obs_event(
        &state,
        "code247.linear.webhook.received",
        &request_id,
        None,
        None,
        issue_id.clone(),
        json!({
            "delivery_id": delivery_id,
            "linear_event": linear_event,
            "deduped": !enqueued,
            "status": status,
        }),
    );

    success_envelope(
        StatusCode::OK,
        &request_id,
        json!(LinearWebhookAck {
            request_id: request_id.clone(),
            accepted: true,
            deduped: !enqueued,
            delivery_id,
            status: status.to_string(),
            issue_id,
        }),
    )
}

async fn post_intentions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<IntentionIntakeRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = validate_intentions_payload(&input, &request_id) {
        return response;
    }
    if let Some(response) = ensure_intentions_auth(
        &state,
        &headers,
        &request_id,
        &state.scope_intentions_write,
        Some((&input.manifest.workspace, &input.manifest.project)),
    ) {
        return response;
    }

    let incoming_updated_at = match DateTime::parse_from_rfc3339(&input.manifest.updated_at) {
        Ok(value) => value.with_timezone(&Utc),
        Err(err) => {
            return error_envelope(
                StatusCode::BAD_REQUEST,
                &request_id,
                "VALIDATION_ERROR",
                "manifest.updated_at deve estar em ISO-8601",
                Some(json!({ "reason": err.to_string() })),
            );
        }
    };

    let previous_ingestion = {
        let store = state
            .manifest_ingestion_store
            .lock()
            .expect("manifest_ingestion_store lock");
        store.get(&input.manifest.workspace, &input.manifest.project)
    };

    if let Some(previous) = previous_ingestion {
        match DateTime::parse_from_rfc3339(&previous.last_updated_at) {
            Ok(previous_updated_at) => {
                let previous_utc = previous_updated_at.with_timezone(&Utc);
                if incoming_updated_at < previous_utc {
                    return error_envelope(
                        StatusCode::CONFLICT,
                        &request_id,
                        "STALE_MANIFEST",
                        "manifest.updated_at é mais antigo que o último ingest",
                        Some(json!({ "last_updated_at": previous.last_updated_at })),
                    );
                }
                if incoming_updated_at == previous_utc {
                    let dedupe = dedupe_response_if_fully_linked(&state, &input, &request_id);
                    if let Some(response) = dedupe {
                        emit_obs_event(
                            &state,
                            "code247.intentions.ingested",
                            &request_id,
                            None,
                            None,
                            None,
                            json!({
                                "workspace": input.manifest.workspace,
                                "project": input.manifest.project,
                                "source": input.source,
                                "revision": input.revision,
                                "deduped": true,
                                "intention_count": input.manifest.intentions.len(),
                            }),
                        );
                        return response;
                    }
                }
            }
            Err(err) => {
                warn!(error=%err, "failed to parse previous manifest updated_at");
            }
        }
    }

    let Some(linear_token) = resolve_linear_token(&state) else {
        return error_envelope(
            StatusCode::SERVICE_UNAVAILABLE,
            &request_id,
            "LINEAR_AUTH_UNAVAILABLE",
            "configure OAuth (recommended) ou LINEAR_API_KEY para sincronizar com Linear",
            None,
        );
    };

    let linear = LinearAdapter::new(
        linear_token,
        state.linear_team_id.clone(),
        state.linear_api_base_url.clone(),
    );
    let mut links = Vec::with_capacity(input.manifest.intentions.len());

    for intention in &input.manifest.intentions {
        let existing_link = {
            let store = state
                .intention_link_store
                .lock()
                .expect("intention_link_store lock");
            store.get_link(
                &input.manifest.workspace,
                &input.manifest.project,
                &intention.id,
            )
        };

        let title = format!("[{}] {}", input.manifest.project, intention.title.trim());
        let description =
            build_intention_description(&state.public_url, &request_id, &input, intention);
        let priority = linear_priority_from_manifest(intention.priority.as_deref());

        let issue_ref = if let Some(link) = existing_link {
            match linear
                .update_issue(&link.linear_issue_id, &title, &description, priority)
                .await
            {
                Ok(issue) => issue,
                Err(err) => {
                    warn!(
                        intention_id=%intention.id,
                        issue_id=%link.linear_issue_id,
                        error=%err,
                        "issue update failed; creating a new issue for reconciliation"
                    );
                    match linear.create_issue(&title, &description, priority).await {
                        Ok(issue) => issue,
                        Err(create_err) => {
                            return error_envelope(
                                StatusCode::BAD_GATEWAY,
                                &request_id,
                                "LINEAR_SYNC_ERROR",
                                "falha ao sincronizar intenção com Linear",
                                Some(json!({
                                    "intention_id": intention.id,
                                    "update_error": err.to_string(),
                                    "create_error": create_err.to_string(),
                                })),
                            );
                        }
                    }
                }
            }
        } else {
            match linear.create_issue(&title, &description, priority).await {
                Ok(issue) => issue,
                Err(err) => {
                    return error_envelope(
                        StatusCode::BAD_GATEWAY,
                        &request_id,
                        "LINEAR_SYNC_ERROR",
                        "falha ao criar issue no Linear",
                        Some(json!({
                            "intention_id": intention.id,
                            "error": err.to_string(),
                        })),
                    );
                }
            }
        };

        let upsert_link_result = {
            let store = state
                .intention_link_store
                .lock()
                .expect("intention_link_store lock");
            store.upsert_link(
                &input.manifest.workspace,
                &input.manifest.project,
                &intention.id,
                &issue_ref.id,
                Some(&issue_ref.identifier),
                &input.manifest.updated_at,
                input.revision.as_deref(),
            )
        };
        if let Err(err) = upsert_link_result {
            return error_envelope(
                StatusCode::INTERNAL_SERVER_ERROR,
                &request_id,
                "PERSISTENCE_ERROR",
                "falha ao persistir linkage intenção->Linear",
                Some(json!({
                    "intention_id": intention.id,
                    "error": err.to_string(),
                })),
            );
        }

        links.push(IntentionLinearLink {
            id: intention.id.clone(),
            issue_id: issue_ref.identifier,
            board: input.manifest.project.clone(),
        });
    }

    let ingestion_upsert = {
        let store = state
            .manifest_ingestion_store
            .lock()
            .expect("manifest_ingestion_store lock");
        store.upsert(
            &input.manifest.workspace,
            &input.manifest.project,
            &input.manifest.updated_at,
            input.revision.as_deref(),
            &request_id,
        )
    };
    if let Err(err) = ingestion_upsert {
        return error_envelope(
            StatusCode::INTERNAL_SERVER_ERROR,
            &request_id,
            "PERSISTENCE_ERROR",
            "falha ao registrar estado de ingestão",
            Some(json!({ "error": err.to_string() })),
        );
    }

    let ci_jobs = input
        .ci_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let queue_id = format!("q-{}", &request_id[..8]);

    if let Err(err) = persist_linear_meta_snapshot(
        &state.linear_meta_path,
        &request_id,
        &input,
        &links,
        &queue_id,
    ) {
        warn!(error=%err, path=%state.linear_meta_path, "failed to write linear meta snapshot");
    }

    info!(
        request_id=%request_id,
        workspace=%input.manifest.workspace,
        project=%input.manifest.project,
        intention_count=input.manifest.intentions.len(),
        "intentions ingested and synchronized with Linear"
    );

    let response = IntentionIntakeResponse {
        request_id,
        deduped: false,
        linear: IntentionLinearResponse { intentions: links },
        ci: IntentionCiResponse {
            jobs: ci_jobs,
            queue_id,
        },
    };
    emit_obs_event(
        &state,
        "code247.intentions.ingested",
        &response.request_id,
        None,
        None,
        None,
        json!({
            "workspace": input.manifest.workspace,
            "project": input.manifest.project,
            "source": input.source,
            "revision": input.revision,
            "deduped": false,
            "intention_count": input.manifest.intentions.len(),
            "linear_issue_ids": response
                .linear
                .intentions
                .iter()
                .map(|link| link.issue_id.clone())
                .collect::<Vec<_>>(),
            "queue_id": response.ci.queue_id.clone(),
            "ci_jobs": response.ci.jobs.clone(),
        }),
    );
    success_envelope(StatusCode::OK, &response.request_id, json!(response))
}

async fn post_intentions_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<IntentionSyncRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if input.workspace.trim().is_empty() || input.project.trim().is_empty() {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "workspace e project são obrigatórios",
            None,
        );
    }
    if input.results.is_empty() {
        return error_envelope(
            StatusCode::BAD_REQUEST,
            &request_id,
            "VALIDATION_ERROR",
            "results não pode ser vazio",
            None,
        );
    }
    if let Some(response) = ensure_intentions_auth(
        &state,
        &headers,
        &request_id,
        &state.scope_intentions_sync,
        Some((&input.workspace, &input.project)),
    ) {
        return response;
    }

    let Some(linear_token) = resolve_linear_token(&state) else {
        return error_envelope(
            StatusCode::SERVICE_UNAVAILABLE,
            &request_id,
            "LINEAR_AUTH_UNAVAILABLE",
            "configure OAuth (recommended) ou LINEAR_API_KEY para sincronizar com Linear",
            None,
        );
    };
    let linear = LinearAdapter::new(
        linear_token,
        state.linear_team_id.clone(),
        state.linear_api_base_url.clone(),
    );

    let mut done_state_cache: Option<String> = None;
    let mut ready_for_release_state_cache: Option<String> = None;
    let mut synced = Vec::new();
    let mut errors = Vec::new();

    for result in &input.results {
        if result.intention_id.trim().is_empty() {
            errors.push(IntentionSyncErrorOutput {
                intention_id: result.intention_id.clone(),
                code: "VALIDATION_ERROR".to_string(),
                message: "intention_id é obrigatório".to_string(),
            });
            continue;
        }

        let link = {
            let store = state
                .intention_link_store
                .lock()
                .expect("intention_link_store lock");
            store.get_link(&input.workspace, &input.project, &result.intention_id)
        };
        let Some(link) = link else {
            errors.push(IntentionSyncErrorOutput {
                intention_id: result.intention_id.clone(),
                code: "LINK_NOT_FOUND".to_string(),
                message: "intenção não encontrada no linkage local".to_string(),
            });
            continue;
        };

        let issue = match linear.get_issue(&link.linear_issue_id).await {
            Ok(item) => item,
            Err(err) => {
                errors.push(IntentionSyncErrorOutput {
                    intention_id: result.intention_id.clone(),
                    code: "LINEAR_ISSUE_ERROR".to_string(),
                    message: format!(
                        "falha ao obter issue '{}' no Linear: {}",
                        link.linear_issue_id, err
                    ),
                });
                continue;
            }
        };
        let current_workflow_state = classify_linear_workflow_state(
            &issue.state.name,
            &issue.state.r#type,
            &state.linear_claim_state_name,
            &state.linear_claim_in_progress_state_name,
            &state.linear_ready_for_release_state_name,
            &state.linear_done_state_type,
        );

        let comment = build_sync_comment(&request_id, &input, result);
        if let Err(err) = linear.create_comment(&link.linear_issue_id, &comment).await {
            errors.push(IntentionSyncErrorOutput {
                intention_id: result.intention_id.clone(),
                code: "LINEAR_COMMENT_ERROR".to_string(),
                message: err.to_string(),
            });
            continue;
        }

        let mut moved_to_done = false;
        let mut moved_to_ready_for_release = false;
        let mut target_state: Option<String> = None;
        let transition = evaluate_sync_transition_request(result, current_workflow_state);
        if let Some(block) = &transition.block {
            let block_message = build_transition_block_message(
                block.code,
                &issue.state.name,
                transition.target_state,
                &state.linear_ready_for_release_state_name,
            );
            errors.push(IntentionSyncErrorOutput {
                intention_id: result.intention_id.clone(),
                code: block.code.to_string(),
                message: block_message.clone(),
            });
            emit_sync_transition_block_event(
                &state,
                &request_id,
                &input.workspace,
                &input.project,
                &result.intention_id,
                link.linear_identifier
                    .clone()
                    .unwrap_or_else(|| link.linear_issue_id.clone()),
                current_workflow_state,
                transition.target_state,
                block.code,
                &block_message,
                transition.requested_transition,
                transition.has_ci_evidence,
                transition.has_deploy_evidence,
            );
            if block.hard_block {
                continue;
            }
        }

        let should_move_done = transition.target_state == Some(LinearWorkflowState::Done);
        let should_move_ready_for_release =
            transition.target_state == Some(LinearWorkflowState::ReadyForRelease);

        if should_move_done {
            if current_workflow_state == LinearWorkflowState::Done {
                target_state = Some(issue.state.name.clone());
                moved_to_done = false;
                synced.push(IntentionSyncResultOutput {
                    intention_id: result.intention_id.clone(),
                    issue_id: link.linear_identifier.unwrap_or(link.linear_issue_id),
                    comment_posted: true,
                    moved_to_ready_for_release,
                    moved_to_done,
                    target_state,
                });
                continue;
            }
            if done_state_cache.is_none() {
                match linear
                    .find_state_id_by_type(&state.linear_done_state_type)
                    .await
                {
                    Ok(state_id) => {
                        done_state_cache = Some(state_id);
                    }
                    Err(err) => {
                        errors.push(IntentionSyncErrorOutput {
                            intention_id: result.intention_id.clone(),
                            code: "LINEAR_STATE_ERROR".to_string(),
                            message: err.to_string(),
                        });
                        continue;
                    }
                }
            }

            if let Some(done_state_id) = done_state_cache.as_deref() {
                if let Err(err) = linear
                    .update_issue_state(&link.linear_issue_id, done_state_id)
                    .await
                {
                    errors.push(IntentionSyncErrorOutput {
                        intention_id: result.intention_id.clone(),
                        code: "LINEAR_STATE_ERROR".to_string(),
                        message: err.to_string(),
                    });
                    continue;
                }
                moved_to_done = true;
                target_state = Some("Done".to_string());
            }
        } else if should_move_ready_for_release {
            if current_workflow_state == LinearWorkflowState::ReadyForRelease {
                target_state = Some(issue.state.name.clone());
                moved_to_ready_for_release = false;
                synced.push(IntentionSyncResultOutput {
                    intention_id: result.intention_id.clone(),
                    issue_id: link.linear_identifier.unwrap_or(link.linear_issue_id),
                    comment_posted: true,
                    moved_to_ready_for_release,
                    moved_to_done,
                    target_state,
                });
                continue;
            }
            if ready_for_release_state_cache.is_none() {
                match linear
                    .find_state_id_by_name(&state.linear_ready_for_release_state_name)
                    .await
                {
                    Ok(state_id) => {
                        ready_for_release_state_cache = Some(state_id);
                    }
                    Err(err) => {
                        errors.push(IntentionSyncErrorOutput {
                            intention_id: result.intention_id.clone(),
                            code: "LINEAR_STATE_ERROR".to_string(),
                            message: format!(
                                "estado '{}' não encontrado: {}",
                                state.linear_ready_for_release_state_name, err
                            ),
                        });
                        continue;
                    }
                }
            }

            if let Some(ready_state_id) = ready_for_release_state_cache.as_deref() {
                if let Err(err) = linear
                    .update_issue_state(&link.linear_issue_id, ready_state_id)
                    .await
                {
                    errors.push(IntentionSyncErrorOutput {
                        intention_id: result.intention_id.clone(),
                        code: "LINEAR_STATE_ERROR".to_string(),
                        message: err.to_string(),
                    });
                    continue;
                }
                moved_to_ready_for_release = true;
                target_state = Some(state.linear_ready_for_release_state_name.clone());
            }
        }

        synced.push(IntentionSyncResultOutput {
            intention_id: result.intention_id.clone(),
            issue_id: link.linear_identifier.unwrap_or(link.linear_issue_id),
            comment_posted: true,
            moved_to_ready_for_release,
            moved_to_done,
            target_state,
        });
    }

    if let Err(err) = persist_linear_meta_sync(
        &state.linear_meta_path,
        &request_id,
        &input.workspace,
        &input.project,
        &synced,
        &errors,
    ) {
        warn!(error=%err, path=%state.linear_meta_path, "failed to persist sync snapshot");
    }

    info!(
        request_id=%request_id,
        workspace=%input.workspace,
        project=%input.project,
        synced_count=synced.len(),
        errors_count=errors.len(),
        "intentions sync processed"
    );

    let primary_intention_id = synced.first().map(|item| item.intention_id.clone());
    let primary_issue_id = synced.first().map(|item| item.issue_id.clone());
    emit_obs_event(
        &state,
        "code247.intentions.synced",
        &request_id,
        primary_intention_id,
        None,
        primary_issue_id,
        json!({
            "workspace": input.workspace.clone(),
            "project": input.project.clone(),
            "synced_count": synced.len(),
            "errors_count": errors.len(),
            "synced": synced
                .iter()
                .map(|item| json!({
                    "intention_id": item.intention_id.clone(),
                    "issue_id": item.issue_id.clone(),
                    "moved_to_ready_for_release": item.moved_to_ready_for_release,
                    "moved_to_done": item.moved_to_done,
                    "target_state": item.target_state.clone(),
                }))
                .collect::<Vec<_>>(),
            "errors": errors
                .iter()
                .map(|item| json!({
                    "intention_id": item.intention_id.clone(),
                    "code": item.code.clone(),
                }))
                .collect::<Vec<_>>(),
        }),
    );

    let response = IntentionSyncResponse {
        request_id: request_id.clone(),
        workspace: input.workspace,
        project: input.project,
        synced,
        errors,
    };
    success_envelope(StatusCode::OK, &request_id, json!(response))
}

async fn get_intentions_snapshot(
    State(state): State<AppState>,
    Path((workspace, project)): Path<(String, String)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4().to_string();
    if let Some(response) = ensure_intentions_auth(
        &state,
        &headers,
        &request_id,
        &state.scope_intentions_read,
        Some((&workspace, &project)),
    ) {
        return response;
    }

    let ingestion = {
        let store = state
            .manifest_ingestion_store
            .lock()
            .expect("manifest_ingestion_store lock");
        store.get(&workspace, &project)
    };
    let links = {
        let store = state
            .intention_link_store
            .lock()
            .expect("intention_link_store lock");
        store.list_project_links(&workspace, &project)
    };

    let serialized_links = links
        .into_iter()
        .map(|entry| IntentionLinearLink {
            id: entry.intention_id,
            issue_id: entry.linear_identifier.unwrap_or(entry.linear_issue_id),
            board: project.clone(),
        })
        .collect::<Vec<_>>();

    let ingestion_json = match ingestion {
        Some(item) => json!({
            "last_updated_at": item.last_updated_at,
            "last_revision": item.last_revision,
            "last_request_id": item.last_request_id,
            "updated_at": item.updated_at,
        }),
        None => json!({}),
    };

    let response = IntentionLinksSnapshotResponse {
        request_id: request_id.clone(),
        workspace,
        project,
        ingestion: ingestion_json,
        links: serialized_links,
    };
    success_envelope(StatusCode::OK, &request_id, json!(response))
}

fn ensure_intentions_auth(
    state: &AppState,
    headers: &HeaderMap,
    request_id: &str,
    required_scope: &str,
    project_scope: Option<(&str, &str)>,
) -> Option<axum::response::Response> {
    let auth_ctx = match validate_bearer_token(state, headers) {
        Ok(ctx) => ctx,
        Err(err) => {
            let (status, code, message) = match err {
                AuthValidationFailure::Config(msg) => {
                    (StatusCode::SERVICE_UNAVAILABLE, "CONFIG_ERROR", msg)
                }
                AuthValidationFailure::Unauthorized(msg) => {
                    (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg)
                }
            };
            return Some(error_envelope(status, request_id, code, &message, None));
        }
    };

    let has_admin_scope = auth_ctx.has_scope(&state.scope_admin);
    if !required_scope.trim().is_empty() && !has_admin_scope && !auth_ctx.has_scope(required_scope)
    {
        return Some(error_envelope(
            StatusCode::FORBIDDEN,
            request_id,
            "FORBIDDEN",
            &format!("scope obrigatório ausente: {}", required_scope.trim()),
            None,
        ));
    }

    if let Some((workspace, project)) = project_scope {
        let workspace = workspace.trim();
        let project = project.trim();
        if !workspace.is_empty()
            && !project.is_empty()
            && !has_admin_scope
            && !auth_ctx.allows_project(workspace, project)
        {
            return Some(error_envelope(
                StatusCode::FORBIDDEN,
                request_id,
                "FORBIDDEN",
                &format!("token sem permissão para projeto '{workspace}/{project}'"),
                None,
            ));
        }
    }

    None
}

fn ensure_admin_auth(
    state: &AppState,
    headers: &HeaderMap,
    request_id: &str,
    required_scope: &str,
) -> Option<axum::response::Response> {
    let auth_ctx = match validate_bearer_token(state, headers) {
        Ok(ctx) => ctx,
        Err(err) => {
            let (status, code, message) = match err {
                AuthValidationFailure::Config(msg) => {
                    (StatusCode::SERVICE_UNAVAILABLE, "CONFIG_ERROR", msg)
                }
                AuthValidationFailure::Unauthorized(msg) => {
                    (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg)
                }
            };
            return Some(error_envelope(status, request_id, code, &message, None));
        }
    };

    let has_admin_scope = auth_ctx.has_scope(&state.scope_admin);
    if !has_admin_scope {
        return Some(error_envelope(
            StatusCode::FORBIDDEN,
            request_id,
            "FORBIDDEN",
            "rota administrativa requer scope code247:admin",
            None,
        ));
    }

    if !required_scope.trim().is_empty() && !auth_ctx.has_scope(required_scope) {
        return Some(error_envelope(
            StatusCode::FORBIDDEN,
            request_id,
            "FORBIDDEN",
            &format!("scope obrigatório ausente: {}", required_scope.trim()),
            None,
        ));
    }

    None
}

#[derive(Debug)]
enum AuthValidationFailure {
    Config(String),
    Unauthorized(String),
}

#[derive(Debug, Default)]
struct AuthContext {
    scopes: HashSet<String>,
    project_grants: HashSet<String>,
}

impl AuthContext {
    fn has_scope(&self, scope: &str) -> bool {
        let normalized = scope.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return true;
        }
        self.scopes.contains("*")
            || self.scopes.contains("code247:*")
            || self.scopes.contains(&normalized)
    }

    fn allows_project(&self, workspace: &str, project: &str) -> bool {
        if self.project_grants.contains("*") {
            return true;
        }
        let workspace = workspace.trim().to_ascii_lowercase();
        let project = project.trim().to_ascii_lowercase();
        if workspace.is_empty() || project.is_empty() {
            return false;
        }
        let exact = format!("{workspace}/{project}");
        let workspace_wildcard = format!("{workspace}/*");
        self.project_grants.contains(&exact) || self.project_grants.contains(&workspace_wildcard)
    }
}

fn validate_bearer_token(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthContext, AuthValidationFailure> {
    let Some(token) = extract_bearer_token(headers) else {
        return Err(AuthValidationFailure::Unauthorized(
            "Authorization Bearer token obrigatório".to_string(),
        ));
    };

    if let Some(claims) = decode_supabase_claims(state, &token) {
        return Ok(auth_context_from_claims(&claims));
    }

    if state.auth_allow_legacy_token {
        if let Some(expected_token) = state
            .intentions_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if token == expected_token {
                let mut ctx = AuthContext::default();
                ctx.scopes.insert("*".to_string());
                ctx.project_grants.insert("*".to_string());
                return Ok(ctx);
            }
        }
    }

    let jwt_configured = state
        .supabase_jwt_secret
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || state
            .supabase_jwt_secret_legacy
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
    let legacy_configured = state
        .intentions_token
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    if !jwt_configured && (!state.auth_allow_legacy_token || !legacy_configured) {
        return Err(AuthValidationFailure::Config(
            "nenhuma auth configurada: definir SUPABASE_JWT_SECRET ou habilitar token legado"
                .to_string(),
        ));
    }

    Err(AuthValidationFailure::Unauthorized(
        "token inválido".to_string(),
    ))
}

fn decode_supabase_claims(state: &AppState, token: &str) -> Option<Value> {
    let audience = state
        .supabase_jwt_audience
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let secrets = [
        state
            .supabase_jwt_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        state
            .supabase_jwt_secret_legacy
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    ];

    for secret in secrets.into_iter().flatten() {
        if let Some(claims) = decode_supabase_claims_with_secret(token, secret, audience) {
            return Some(claims);
        }
    }

    None
}

fn decode_supabase_claims_with_secret(
    token: &str,
    secret: &str,
    audience: Option<&str>,
) -> Option<Value> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 30;
    if let Some(aud) = audience {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }

    jsonwebtoken::decode::<Value>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .ok()
    .map(|decoded| decoded.claims)
}

fn auth_context_from_claims(claims: &Value) -> AuthContext {
    let mut scopes = HashSet::new();
    collect_scope_claim_values(claims.get("scope"), &mut scopes);
    collect_scope_claim_values(claims.get("scopes"), &mut scopes);
    collect_scope_claim_values(claims.pointer("/app_metadata/scope"), &mut scopes);
    collect_scope_claim_values(claims.pointer("/app_metadata/scopes"), &mut scopes);

    let mut project_grants = HashSet::new();
    collect_project_grants(claims.get("code247_projects"), &mut project_grants);
    collect_project_grants(claims.get("projects"), &mut project_grants);
    collect_project_grants(
        claims.pointer("/app_metadata/code247_projects"),
        &mut project_grants,
    );
    collect_project_grants(
        claims.pointer("/app_metadata/projects"),
        &mut project_grants,
    );

    for scope in &scopes {
        if let Some(grant) = scope.strip_prefix("code247:project:") {
            if let Some(normalized) = normalize_project_grant(grant) {
                project_grants.insert(normalized);
            }
        }
    }

    AuthContext {
        scopes,
        project_grants,
    }
}

fn collect_scope_claim_values(raw: Option<&Value>, output: &mut HashSet<String>) {
    let mut values = Vec::new();
    collect_claim_tokens(raw, &mut values);
    for scope in values {
        let normalized = scope.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            output.insert(normalized);
        }
    }
}

fn collect_project_grants(raw: Option<&Value>, output: &mut HashSet<String>) {
    let mut values = Vec::new();
    collect_claim_tokens(raw, &mut values);
    for value in values {
        if let Some(normalized) = normalize_project_grant(&value) {
            output.insert(normalized);
        }
    }
}

fn collect_claim_tokens(raw: Option<&Value>, output: &mut Vec<String>) {
    let Some(raw) = raw else {
        return;
    };
    match raw {
        Value::String(value) => {
            output.extend(
                value
                    .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
                    .map(str::trim)
                    .filter(|token| !token.is_empty())
                    .map(ToString::to_string),
            );
        }
        Value::Array(values) => {
            for value in values {
                if let Some(item) = value.as_str() {
                    output.push(item.to_string());
                }
            }
        }
        _ => {}
    }
}

fn normalize_project_grant(raw: &str) -> Option<String> {
    let mut normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized == "*" {
        return Some(normalized);
    }
    if let Some(stripped) = normalized.strip_prefix("code247:project:") {
        normalized = stripped.to_string();
    }
    if normalized == "*" {
        return Some(normalized);
    }

    let mut segments = normalized.split('/');
    let workspace = segments.next()?.trim();
    let project = segments.next()?.trim();
    if segments.next().is_some() || workspace.is_empty() || project.is_empty() {
        return None;
    }

    Some(format!("{workspace}/{project}"))
}

fn validate_intentions_payload(
    input: &IntentionIntakeRequest,
    request_id: &str,
) -> Option<axum::response::Response> {
    if input.manifest.workspace.trim().is_empty() {
        return Some(error_envelope(
            StatusCode::BAD_REQUEST,
            request_id,
            "VALIDATION_ERROR",
            "manifest.workspace é obrigatório",
            None,
        ));
    }
    if input.manifest.project.trim().is_empty() {
        return Some(error_envelope(
            StatusCode::BAD_REQUEST,
            request_id,
            "VALIDATION_ERROR",
            "manifest.project é obrigatório",
            None,
        ));
    }
    if input.manifest.updated_at.trim().is_empty() {
        return Some(error_envelope(
            StatusCode::BAD_REQUEST,
            request_id,
            "VALIDATION_ERROR",
            "manifest.updated_at é obrigatório",
            None,
        ));
    }
    if input.source.trim().is_empty() {
        return Some(error_envelope(
            StatusCode::BAD_REQUEST,
            request_id,
            "VALIDATION_ERROR",
            "source é obrigatório",
            None,
        ));
    }
    for intention in &input.manifest.intentions {
        if intention.id.trim().is_empty() || intention.title.trim().is_empty() {
            return Some(error_envelope(
                StatusCode::BAD_REQUEST,
                request_id,
                "VALIDATION_ERROR",
                "cada intenção precisa de id e title",
                Some(json!({"intention": intention.id})),
            ));
        }
        for task in &intention.tasks {
            if task.description.trim().is_empty() {
                return Some(error_envelope(
                    StatusCode::BAD_REQUEST,
                    request_id,
                    "VALIDATION_ERROR",
                    "task.description não pode ser vazio",
                    Some(json!({"intention": intention.id})),
                ));
            }
        }
    }
    None
}

fn dedupe_response_if_fully_linked(
    state: &AppState,
    input: &IntentionIntakeRequest,
    request_id: &str,
) -> Option<axum::response::Response> {
    let mut links = Vec::with_capacity(input.manifest.intentions.len());
    {
        let store = state
            .intention_link_store
            .lock()
            .expect("intention_link_store lock");
        for intention in &input.manifest.intentions {
            let Some(link) = store.get_link(
                &input.manifest.workspace,
                &input.manifest.project,
                &intention.id,
            ) else {
                return None;
            };
            links.push(IntentionLinearLink {
                id: intention.id.clone(),
                issue_id: link.linear_identifier.unwrap_or(link.linear_issue_id),
                board: input.manifest.project.clone(),
            });
        }
    }

    let ci_jobs = input
        .ci_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_string()])
        .unwrap_or_default();
    let queue_id = format!("q-{}", &request_id[..8]);

    let response = IntentionIntakeResponse {
        request_id: request_id.to_string(),
        deduped: true,
        linear: IntentionLinearResponse { intentions: links },
        ci: IntentionCiResponse {
            jobs: ci_jobs,
            queue_id,
        },
    };
    Some(success_envelope(
        StatusCode::OK,
        &response.request_id,
        json!(response),
    ))
}

fn resolve_linear_token(state: &AppState) -> Option<String> {
    if let Some(token) = state
        .oauth_token_store
        .lock()
        .expect("oauth_token_store lock")
        .get_token()
        .map(|value| value.access_token)
    {
        return Some(token);
    }

    state
        .linear_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn linear_priority_from_manifest(priority: Option<&str>) -> i32 {
    match priority.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "critical" => 1,
        "high" => 2,
        "medium" => 3,
        "low" => 4,
        _ => 0,
    }
}

fn has_ci_evidence(result: &IntentionSyncResultInput) -> bool {
    let ci_has_url = result
        .ci
        .as_ref()
        .and_then(|ci| ci.url.as_deref())
        .map(str::trim)
        .map(|url| !url.is_empty())
        .unwrap_or(false);

    let has_evidence_links = result
        .evidence
        .iter()
        .any(|item| !item.label.trim().is_empty() && !item.url.trim().is_empty());

    ci_has_url || has_evidence_links
}

fn has_deploy_evidence(result: &IntentionSyncResultInput) -> bool {
    if result
        .evidence
        .iter()
        .any(|item| is_deploy_evidence(item.label.trim(), item.url.trim()))
    {
        return true;
    }

    let ci_job_or_queue_deploy_hint = result
        .ci
        .as_ref()
        .map(|ci| {
            [ci.job.as_deref(), ci.queue_id.as_deref()]
                .into_iter()
                .flatten()
                .map(str::trim)
                .any(contains_deploy_hint)
        })
        .unwrap_or(false);
    if ci_job_or_queue_deploy_hint {
        return true;
    }

    result
        .ci
        .as_ref()
        .and_then(|ci| ci.url.as_deref())
        .map(str::trim)
        .is_some_and(contains_deploy_hint)
}

fn is_deploy_evidence(label: &str, url: &str) -> bool {
    if label.is_empty() || url.is_empty() {
        return false;
    }
    contains_deploy_hint(label) || contains_deploy_hint(url)
}

fn contains_deploy_hint(raw: &str) -> bool {
    let normalized = raw.to_ascii_lowercase();
    [
        "deploy",
        "release",
        "rollout",
        "production",
        "prod",
        "vercel",
        "cloudflare",
        "fly.io",
        "render.com",
        "kubernetes",
        "k8s",
    ]
    .iter()
    .any(|token| normalized.contains(token))
}

fn evaluate_sync_transition_request(
    result: &IntentionSyncResultInput,
    current_state: LinearWorkflowState,
) -> crate::transition_guard_rs::SyncTransitionEvaluation {
    let requested_transition =
        result.status.eq_ignore_ascii_case("success") && result.set_done_on_success.unwrap_or(true);
    let has_ci_evidence = has_ci_evidence(result);
    let has_deploy_evidence = has_deploy_evidence(result);
    evaluate_transition(
        requested_transition,
        has_ci_evidence,
        has_deploy_evidence,
        current_state,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_sync_transition_block_event(
    state: &AppState,
    request_id: &str,
    workspace: &str,
    project: &str,
    intention_id: &str,
    issue_id: String,
    from_state: LinearWorkflowState,
    to_state: Option<LinearWorkflowState>,
    reason_code: &str,
    message: &str,
    requested_transition: bool,
    has_ci_evidence: bool,
    has_deploy_evidence: bool,
) {
    let from_state_label =
        workflow_state_label(from_state, &state.linear_ready_for_release_state_name);
    let to_state_label = to_state
        .map(|value| workflow_state_label(value, &state.linear_ready_for_release_state_name));

    emit_obs_event(
        state,
        "code247.intentions.sync.transition_blocked",
        request_id,
        Some(intention_id.to_string()),
        None,
        Some(issue_id),
        json!({
            "workspace": workspace,
            "project": project,
            "reason_code": reason_code,
            "message": message,
            "from_state": from_state_label,
            "to_state": to_state_label,
            "requested_transition": requested_transition,
            "has_ci_evidence": has_ci_evidence,
            "has_deploy_evidence": has_deploy_evidence,
        }),
    );
}

fn build_intention_description(
    public_url: &str,
    request_id: &str,
    input: &IntentionIntakeRequest,
    intention: &IntentionRecord,
) -> String {
    let mut lines = vec![
        format!("# {}", intention.title.trim()),
        String::new(),
        format!("- workspace: `{}`", input.manifest.workspace),
        format!("- project: `{}`", input.manifest.project),
        format!("- intention_id: `{}`", intention.id),
        format!("- source: `{}`", input.source),
        format!(
            "- revision: `{}`",
            input.revision.as_deref().unwrap_or("unknown")
        ),
        format!("- manifest_updated_at: `{}`", input.manifest.updated_at),
        format!("- request_id: `{}`", request_id),
        format!("- intake: `{}`", public_url.trim_end_matches('/')),
    ];

    if let Some(kind) = intention
        .r#type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        lines.push(format!("- type: `{kind}`"));
    }
    if let Some(scope) = intention
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        lines.push(format!("- scope: `{scope}`"));
    }
    if let Some(priority) = intention
        .priority
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        lines.push(format!("- priority: `{priority}`"));
    }

    if !intention.tasks.is_empty() {
        lines.push(String::new());
        lines.push("## Tasks".to_string());
        for task in &intention.tasks {
            let mut details = Vec::new();
            if let Some(owner) = task
                .owner
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                details.push(format!("owner: {owner}"));
            }
            if let Some(due) = task.due.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                details.push(format!("due: {due}"));
            }
            if let Some(gate) = task
                .gate
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                details.push(format!("gate: {gate}"));
            }

            if details.is_empty() {
                lines.push(format!("- [ ] {}", task.description.trim()));
            } else {
                lines.push(format!(
                    "- [ ] {} ({})",
                    task.description.trim(),
                    details.join(", ")
                ));
            }
        }
    }

    lines.join("\n")
}

fn persist_linear_meta_snapshot(
    output_path: &str,
    request_id: &str,
    input: &IntentionIntakeRequest,
    links: &[IntentionLinearLink],
    queue_id: &str,
) -> Result<()> {
    let path = PathBuf::from(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let payload = json!({
        "request_id": request_id,
        "workspace": input.manifest.workspace,
        "project": input.manifest.project,
        "updated_at": input.manifest.updated_at,
        "source": input.source,
        "revision": input.revision,
        "linear": {
            "intentions": links,
        },
        "ci": {
            "ci_target": input.ci_target,
            "queue_id": queue_id,
        },
        "observed_at": Utc::now().to_rfc3339(),
    });

    fs::write(path, serde_json::to_string_pretty(&payload)?)?;
    Ok(())
}

fn build_sync_comment(
    request_id: &str,
    input: &IntentionSyncRequest,
    result: &IntentionSyncResultInput,
) -> String {
    let mut lines = vec![
        "## Code247 sync update".to_string(),
        format!("- request_id: `{request_id}`"),
        format!("- workspace: `{}`", input.workspace),
        format!("- project: `{}`", input.project),
        format!("- intention_id: `{}`", result.intention_id),
        format!("- status: `{}`", result.status.trim()),
        format!("- synced_at: `{}`", Utc::now().to_rfc3339()),
    ];

    if let Some(summary) = result
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        lines.push(String::new());
        lines.push("### Summary".to_string());
        lines.push(summary.to_string());
    }

    if let Some(ci) = result.ci.as_ref() {
        lines.push(String::new());
        lines.push("### CI".to_string());
        if let Some(queue_id) = ci
            .queue_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            lines.push(format!("- queue_id: `{queue_id}`"));
        }
        if let Some(job) = ci.job.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            lines.push(format!("- job: `{job}`"));
        }
        if let Some(url) = ci.url.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            lines.push(format!("- url: {url}"));
        }
    }

    if !result.evidence.is_empty() {
        lines.push(String::new());
        lines.push("### Evidence".to_string());
        for evidence in &result.evidence {
            let label = evidence.label.trim();
            let url = evidence.url.trim();
            if !label.is_empty() && !url.is_empty() {
                lines.push(format!("- {label}: {url}"));
            }
        }
    }

    lines.join("\n")
}

fn persist_linear_meta_sync(
    output_path: &str,
    request_id: &str,
    workspace: &str,
    project: &str,
    synced: &[IntentionSyncResultOutput],
    errors: &[IntentionSyncErrorOutput],
) -> Result<()> {
    let path = PathBuf::from(output_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let current = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .unwrap_or_else(|| json!({}));
    let mut merged = current;
    if !merged.is_object() {
        merged = json!({});
    }

    let sync_block = json!({
        "request_id": request_id,
        "workspace": workspace,
        "project": project,
        "synced": synced,
        "errors": errors,
        "observed_at": Utc::now().to_rfc3339(),
    });
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("sync".to_string(), sync_block);
    }

    fs::write(path, serde_json::to_string_pretty(&merged)?)?;
    Ok(())
}

fn emit_obs_event(
    state: &AppState,
    event_type: &str,
    request_id: &str,
    intention_id: Option<String>,
    run_id: Option<String>,
    issue_id: Option<String>,
    payload: Value,
) {
    let Some(base_url) = state
        .obs_api_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let event = json!({
        "event_id": Uuid::new_v4().to_string(),
        "event_type": event_type,
        "occurred_at": Utc::now().to_rfc3339(),
        "source": "code247",
        "request_id": request_id,
        "trace_id": request_id,
        "parent_event_id": Value::Null,
        "intention_id": intention_id,
        "run_id": run_id,
        "issue_id": issue_id,
        "pr_id": Value::Null,
        "deploy_id": Value::Null,
        "payload": payload,
    });

    let url = format!("{}/api/v1/events/ingest", base_url.trim_end_matches('/'));
    let token = state
        .obs_api_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let client = state.obs_api_client.clone();
    let event_type = event_type.to_string();
    let request_id = request_id.to_string();

    tokio::spawn(async move {
        let mut request = client.post(url).json(&event);
        if let Some(token_value) = token {
            request = request.bearer_auth(token_value);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => {
                warn!(
                    event_type=%event_type,
                    request_id=%request_id,
                    status=%response.status(),
                    "obs-api ingest returned non-success"
                );
            }
            Err(err) => {
                warn!(
                    event_type=%event_type,
                    request_id=%request_id,
                    error=%err,
                    "failed to mirror event to obs-api ingest"
                );
            }
        }
    });
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn verify_linear_signature(secret: &str, body: &[u8], provided_signature: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;
    let provided = provided_signature
        .trim()
        .strip_prefix("sha256=")
        .unwrap_or(provided_signature.trim());
    let Ok(provided_bytes) = hex::decode(provided) else {
        return false;
    };

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&provided_bytes).is_ok()
}

fn extract_webhook_timestamp_ms(payload: &Value) -> Option<i64> {
    let raw = payload.get("webhookTimestamp")?;
    if let Some(ms) = raw.as_i64() {
        return Some(ms);
    }
    raw.as_str()?.trim().parse::<i64>().ok()
}

fn extract_webhook_issue_id(payload: &Value) -> Option<String> {
    payload
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .pointer("/data/issue/id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let header = headers.get("authorization")?.to_str().ok()?.trim();
    let token = header.strip_prefix("Bearer ")?;
    let trimmed = token.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn error_envelope(
    status: StatusCode,
    request_id: &str,
    code: &str,
    message: &str,
    details: Option<Value>,
) -> axum::response::Response {
    response_with_request_id(
        status,
        request_id,
        json!({
            "request_id": request_id,
            "output_schema": ERROR_ENVELOPE_SCHEMA,
            "error": {
                "type": code,
                "code": code,
                "message": message,
                "details": details.unwrap_or_else(|| json!({})),
            }
        }),
    )
}

fn success_envelope(
    status: StatusCode,
    request_id: &str,
    payload: Value,
) -> axum::response::Response {
    let mut body = object_payload(payload);
    body.insert(
        "request_id".to_string(),
        Value::String(request_id.to_string()),
    );
    body.insert(
        "output_schema".to_string(),
        Value::String(RESPONSE_ENVELOPE_SCHEMA.to_string()),
    );
    response_with_request_id(status, request_id, Value::Object(body))
}

fn object_payload(payload: Value) -> Map<String, Value> {
    match payload {
        Value::Object(obj) => obj,
        other => {
            let mut obj = Map::new();
            obj.insert("data".to_string(), other);
            obj
        }
    }
}

fn response_with_request_id(
    status: StatusCode,
    request_id: &str,
    body: Value,
) -> axum::response::Response {
    let mut response = (status, Json(body)).into_response();
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::{
        auth_context_from_claims, oauth_start, validate_bearer_token,
        evaluate_sync_transition_request, extract_webhook_timestamp_ms, normalize_project_grant,
        verify_linear_signature, AppState, AuthValidationFailure, IntentionSyncResultInput,
    };
    use crate::persistence_rs::{
        IntentionLinkRepository, JobsRepository, LinearOAuthTokenRepository,
        LinearWebhookDeliveryRepository, ManifestIngestionRepository, OAuthStateRepository,
        RunTimelineRepository, SqliteDb,
    };
    use crate::transition_guard_rs::{
        build_transition_block_message, classify_linear_workflow_state,
        is_linear_transition_allowed, requested_workflow_transition, LinearWorkflowState,
    };
    use axum::{
        body::to_bytes,
        extract::State,
        http::{header, HeaderMap, HeaderValue, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
        Json, Router,
    };
    use hmac::{Hmac, Mac};
    use jsonwebtoken::{EncodingKey, Header};
    use reqwest::Client;
    use serde_json::{json, Value};
    use sha2::Sha256;
    use std::{
        collections::HashMap,
        env,
        sync::{Arc, Mutex},
    };
    use tokio::{net::TcpListener, task::JoinHandle};
    use uuid::Uuid;

    fn test_app_state(auth_allow_legacy_token: bool, jwt_secret: Option<&str>) -> AppState {
        let path = env::temp_dir().join(format!("code247-auth-test-{}.db", Uuid::new_v4()));
        let sqlite = SqliteDb::open(&path.display().to_string()).expect("sqlite open");
        sqlite.run_migrations().expect("sqlite migrations");
        let conn = sqlite.connection();

        AppState {
            jobs: Arc::new(Mutex::new(JobsRepository::new(conn.clone(), None))),
            run_timeline: Arc::new(Mutex::new(RunTimelineRepository::new(conn.clone()))),
            oauth_state_store: Arc::new(Mutex::new(OAuthStateRepository::new(conn.clone()))),
            oauth_token_store: Arc::new(Mutex::new(LinearOAuthTokenRepository::new(conn.clone()))),
            manifest_ingestion_store: Arc::new(Mutex::new(ManifestIngestionRepository::new(
                conn.clone(),
            ))),
            intention_link_store: Arc::new(Mutex::new(IntentionLinkRepository::new(conn.clone()))),
            oauth_client: None,
            oauth_state_ttl_seconds: 600,
            linear_team_id: "team".to_string(),
            linear_claim_state_name: "Ready".to_string(),
            linear_claim_in_progress_state_name: "In Progress".to_string(),
            linear_done_state_type: "completed".to_string(),
            linear_ready_for_release_state_name: "Ready for Release".to_string(),
            linear_api_key: None,
            linear_api_base_url: "https://api.linear.app".to_string(),
            intentions_token: Some("legacy-intentions-token".to_string()),
            auth_allow_legacy_token,
            supabase_jwt_secret: jwt_secret.map(ToString::to_string),
            supabase_jwt_secret_legacy: None,
            supabase_jwt_audience: None,
            scope_jobs_read: "code247:jobs:read".to_string(),
            scope_jobs_write: "code247:jobs:write".to_string(),
            scope_intentions_write: "code247:intentions:write".to_string(),
            scope_intentions_sync: "code247:intentions:sync".to_string(),
            scope_intentions_read: "code247:intentions:read".to_string(),
            scope_admin: "code247:admin".to_string(),
            linear_webhook_secret: Some("linear-secret".to_string()),
            linear_webhook_max_skew_seconds: 60,
            linear_meta_path: ".code247/linear-meta.json".to_string(),
            public_url: "https://code247.logline.world".to_string(),
            obs_api_base_url: None,
            obs_api_token: None,
            obs_api_client: Client::new(),
            webhook_delivery_store: Arc::new(Mutex::new(LinearWebhookDeliveryRepository::new(
                conn,
            ))),
        }
    }

    fn bearer_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).expect("header value"),
        );
        headers
    }

    fn sign_hs256_jwt(secret: &str, claims: serde_json::Value) -> String {
        jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("jwt encode")
    }

    #[test]
    fn validates_linear_signature_hex() {
        type HmacSha256 = Hmac<Sha256>;
        let secret = "secret-123";
        let body = br#"{"hello":"world"}"#;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac");
        mac.update(body);
        let digest = mac.finalize().into_bytes();
        let signature = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(verify_linear_signature(secret, body, &signature));
        assert!(verify_linear_signature(
            secret,
            body,
            &format!("sha256={signature}")
        ));
    }

    #[test]
    fn extracts_webhook_timestamp_number_or_string() {
        let payload_num = json!({"webhookTimestamp": 1741150175000i64});
        let payload_str = json!({"webhookTimestamp": "1741150175000"});
        let payload_invalid = json!({"webhookTimestamp": "oops"});
        assert_eq!(
            extract_webhook_timestamp_ms(&payload_num),
            Some(1741150175000)
        );
        assert_eq!(
            extract_webhook_timestamp_ms(&payload_str),
            Some(1741150175000)
        );
        assert!(extract_webhook_timestamp_ms(&payload_invalid).is_none());
    }

    #[test]
    fn blocks_ready_to_done_transition() {
        assert!(!is_linear_transition_allowed(
            LinearWorkflowState::Ready,
            LinearWorkflowState::Done
        ));
    }

    #[test]
    fn blocks_in_progress_to_done_transition() {
        assert!(!is_linear_transition_allowed(
            LinearWorkflowState::InProgress,
            LinearWorkflowState::Done
        ));
    }

    #[test]
    fn allows_in_progress_to_ready_for_release_transition() {
        assert!(is_linear_transition_allowed(
            LinearWorkflowState::InProgress,
            LinearWorkflowState::ReadyForRelease
        ));
    }

    #[test]
    fn allows_ready_for_release_to_done_transition() {
        assert!(is_linear_transition_allowed(
            LinearWorkflowState::ReadyForRelease,
            LinearWorkflowState::Done
        ));
    }

    #[test]
    fn classifies_workflow_state_using_name_and_type() {
        let ready = classify_linear_workflow_state(
            "Ready",
            "unstarted",
            "Ready",
            "In Progress (Code247)",
            "Ready for Release",
            "completed",
        );
        let in_progress = classify_linear_workflow_state(
            "In Progress (Code247)",
            "started",
            "Ready",
            "In Progress (Code247)",
            "Ready for Release",
            "completed",
        );
        let done = classify_linear_workflow_state(
            "Done",
            "completed",
            "Ready",
            "In Progress (Code247)",
            "Ready for Release",
            "completed",
        );
        assert_eq!(ready, LinearWorkflowState::Ready);
        assert_eq!(in_progress, LinearWorkflowState::InProgress);
        assert_eq!(done, LinearWorkflowState::Done);
    }

    #[test]
    fn requested_transition_to_done_requires_ci_and_deploy_evidence() {
        assert_eq!(
            requested_workflow_transition(true, true, true),
            Some(LinearWorkflowState::Done)
        );
        assert_eq!(requested_workflow_transition(true, false, true), None);
        assert_eq!(
            requested_workflow_transition(true, true, false),
            Some(LinearWorkflowState::ReadyForRelease)
        );
    }

    #[test]
    fn canonical_done_path_requires_ready_for_release() {
        assert!(is_linear_transition_allowed(
            LinearWorkflowState::Ready,
            LinearWorkflowState::InProgress
        ));
        assert!(is_linear_transition_allowed(
            LinearWorkflowState::InProgress,
            LinearWorkflowState::ReadyForRelease
        ));
        assert!(is_linear_transition_allowed(
            LinearWorkflowState::ReadyForRelease,
            LinearWorkflowState::Done
        ));
        assert!(!is_linear_transition_allowed(
            LinearWorkflowState::InProgress,
            LinearWorkflowState::Done
        ));
    }

    #[test]
    fn normalizes_project_grants_with_wildcards() {
        assert_eq!(
            normalize_project_grant("Workspace-A/Project-X"),
            Some("workspace-a/project-x".to_string())
        );
        assert_eq!(
            normalize_project_grant("code247:project:workspace-a/*"),
            Some("workspace-a/*".to_string())
        );
        assert_eq!(normalize_project_grant("*"), Some("*".to_string()));
        assert!(normalize_project_grant("workspace-only").is_none());
    }

    #[test]
    fn reads_scope_and_project_claims_from_supabase_jwt_payload() {
        let claims = json!({
            "scope": "code247:intentions:write,code247:jobs:read",
            "scopes": ["code247:intentions:sync"],
            "code247_projects": ["Workspace-A/Project-X", "workspace-b/*"],
            "app_metadata": {
                "scope": "code247:intentions:read",
                "projects": "workspace-c/project-z"
            }
        });

        let ctx = auth_context_from_claims(&claims);
        assert!(ctx.has_scope("code247:intentions:write"));
        assert!(ctx.has_scope("code247:intentions:sync"));
        assert!(ctx.has_scope("code247:intentions:read"));
        assert!(ctx.allows_project("workspace-a", "project-x"));
        assert!(ctx.allows_project("workspace-b", "any-project"));
        assert!(ctx.allows_project("workspace-c", "project-z"));
        assert!(!ctx.allows_project("workspace-d", "project-z"));
    }

    #[test]
    fn rejects_legacy_token_when_disabled() {
        let state = test_app_state(false, Some("jwt-secret"));
        let headers = bearer_headers("legacy-intentions-token");
        let err = validate_bearer_token(&state, &headers).expect_err("legacy token must fail");
        match err {
            AuthValidationFailure::Config(_) => panic!("expected unauthorized, got config error"),
            AuthValidationFailure::Unauthorized(message) => {
                assert_eq!(message, "token inválido");
            }
        }
    }

    #[tokio::test]
    async fn oauth_start_requires_admin_scope() {
        let state = test_app_state(false, Some("jwt-secret"));
        let user_token = sign_hs256_jwt(
            "jwt-secret",
            json!({
                "sub": "svc-user",
                "exp": 4_102_444_800u64,
                "scope": "code247:jobs:read"
            }),
        );

        let response = oauth_start(State(state), bearer_headers(&user_token))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn oauth_start_accepts_admin_jwt_before_oauth_checks() {
        let state = test_app_state(false, Some("jwt-secret"));
        let admin_token = sign_hs256_jwt(
            "jwt-secret",
            json!({
                "sub": "svc-admin",
                "exp": 4_102_444_800u64,
                "scope": "code247:admin"
            }),
        );

        let response = oauth_start(State(state), bearer_headers(&admin_token))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn eval_transition_soft_blocks_when_ci_evidence_missing() {
        let result = IntentionSyncResultInput {
            intention_id: "abc".to_string(),
            status: "success".to_string(),
            summary: None,
            ci: None,
            evidence: vec![],
            set_done_on_success: Some(true),
        };
        let eval = evaluate_sync_transition_request(&result, LinearWorkflowState::InProgress);
        assert_eq!(eval.target_state, None);
        let block = eval.block.expect("expected block");
        assert_eq!(block.code, "EVIDENCE_REQUIRED");
        assert!(!block.hard_block);
        assert_eq!(
            build_transition_block_message(
                block.code,
                "In Progress (Code247)",
                eval.target_state,
                "Ready for Release",
            ),
            "status=success requer evidência mínima de CI/checks antes de avançar estado"
        );
    }

    #[test]
    fn eval_transition_hard_blocks_invalid_state_change() {
        let result = IntentionSyncResultInput {
            intention_id: "abc".to_string(),
            status: "success".to_string(),
            summary: None,
            ci: Some(super::IntentionSyncCiInput {
                queue_id: Some("q1".to_string()),
                job: Some("deploy-job".to_string()),
                url: Some("https://ci.example/run/1".to_string()),
            }),
            evidence: vec![super::IntentionSyncEvidenceInput {
                label: "deploy".to_string(),
                url: "https://deploy.example/release/1".to_string(),
            }],
            set_done_on_success: Some(true),
        };
        let eval = evaluate_sync_transition_request(&result, LinearWorkflowState::InProgress);
        assert_eq!(eval.target_state, Some(LinearWorkflowState::Done));
        let block = eval.block.expect("expected invalid transition block");
        assert_eq!(block.code, "INVALID_STATE_TRANSITION");
        assert!(block.hard_block);
    }

    #[test]
    fn eval_transition_allows_ready_for_release_to_done() {
        let result = IntentionSyncResultInput {
            intention_id: "abc".to_string(),
            status: "success".to_string(),
            summary: None,
            ci: Some(super::IntentionSyncCiInput {
                queue_id: Some("q1".to_string()),
                job: Some("deploy-job".to_string()),
                url: Some("https://ci.example/run/1".to_string()),
            }),
            evidence: vec![super::IntentionSyncEvidenceInput {
                label: "deploy".to_string(),
                url: "https://deploy.example/release/1".to_string(),
            }],
            set_done_on_success: Some(true),
        };
        let eval = evaluate_sync_transition_request(&result, LinearWorkflowState::ReadyForRelease);
        assert_eq!(eval.target_state, Some(LinearWorkflowState::Done));
        assert!(eval.block.is_none());
    }

    const MOCK_LINEAR_STATE_READY: &str = "state-ready";
    const MOCK_LINEAR_STATE_IN_PROGRESS: &str = "state-in-progress";
    const MOCK_LINEAR_STATE_READY_FOR_RELEASE: &str = "state-ready-for-release";
    const MOCK_LINEAR_STATE_DONE: &str = "state-done";

    #[derive(Clone)]
    struct MockLinearState {
        issues: Arc<Mutex<HashMap<String, MockLinearIssue>>>,
        next_issue: Arc<Mutex<u32>>,
    }

    #[derive(Clone)]
    struct MockLinearIssue {
        id: String,
        identifier: String,
        title: String,
        description: Option<String>,
        state_id: String,
    }

    struct MockLinearServer {
        base_url: String,
        state: MockLinearState,
        handle: JoinHandle<()>,
    }

    impl Drop for MockLinearServer {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    impl MockLinearServer {
        async fn spawn() -> Self {
            let state = MockLinearState {
                issues: Arc::new(Mutex::new(HashMap::new())),
                next_issue: Arc::new(Mutex::new(1)),
            };

            let app = Router::new()
                .route("/graphql", post(mock_linear_graphql))
                .with_state(state.clone());
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("mock linear bind");
            let address = listener.local_addr().expect("mock linear addr");
            let handle = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("mock linear serve");
            });

            Self {
                base_url: format!("http://{}", address),
                state,
                handle,
            }
        }

        fn set_issue_state(&self, issue_id: &str, state_id: &str) {
            let mut issues = self.state.issues.lock().expect("mock issues lock");
            let issue = issues.get_mut(issue_id).expect("mock issue exists");
            issue.state_id = state_id.to_string();
        }

        fn issue_state_name(&self, issue_id: &str) -> String {
            let issues = self.state.issues.lock().expect("mock issues lock");
            let issue = issues.get(issue_id).expect("mock issue exists");
            mock_linear_state_meta(&issue.state_id).1.to_string()
        }
    }

    fn mock_linear_state_meta(state_id: &str) -> (&'static str, &'static str, &'static str) {
        match state_id {
            MOCK_LINEAR_STATE_READY => (MOCK_LINEAR_STATE_READY, "Ready", "unstarted"),
            MOCK_LINEAR_STATE_IN_PROGRESS => (
                MOCK_LINEAR_STATE_IN_PROGRESS,
                "In Progress (Code247)",
                "started",
            ),
            MOCK_LINEAR_STATE_READY_FOR_RELEASE => (
                MOCK_LINEAR_STATE_READY_FOR_RELEASE,
                "Ready for Release",
                "started",
            ),
            MOCK_LINEAR_STATE_DONE => (MOCK_LINEAR_STATE_DONE, "Done", "completed"),
            other => panic!("unknown mock state id: {other}"),
        }
    }

    fn mock_linear_state_json(state_id: &str) -> Value {
        let (id, name, state_type) = mock_linear_state_meta(state_id);
        json!({
            "id": id,
            "name": name,
            "type": state_type,
        })
    }

    async fn mock_linear_graphql(
        State(state): State<MockLinearState>,
        Json(payload): Json<Value>,
    ) -> Json<Value> {
        let query = payload
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let variables = payload.get("variables").cloned().unwrap_or_else(|| json!({}));

        if query.contains("issueCreate(") {
            let mut next_issue = state.next_issue.lock().expect("mock next issue lock");
            let issue_number = *next_issue;
            *next_issue += 1;
            let issue_id = format!("lin-{issue_number}");
            let identifier = format!("VV-{issue_number}");
            let issue = MockLinearIssue {
                id: issue_id.clone(),
                identifier: identifier.clone(),
                title: variables
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                description: variables
                    .get("description")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                state_id: MOCK_LINEAR_STATE_READY.to_string(),
            };
            state
                .issues
                .lock()
                .expect("mock issues lock")
                .insert(issue_id.clone(), issue);
            return Json(json!({
                "data": {
                    "issueCreate": {
                        "success": true,
                        "issue": {
                            "id": issue_id,
                            "identifier": identifier,
                        }
                    }
                }
            }));
        }

        if query.contains("issueUpdate(") && variables.get("stateId").is_some() {
            let issue_id = variables
                .get("id")
                .and_then(Value::as_str)
                .expect("state update issue id");
            let state_id = variables
                .get("stateId")
                .and_then(Value::as_str)
                .expect("state update state id");
            let mut issues = state.issues.lock().expect("mock issues lock");
            let issue = issues.get_mut(issue_id).expect("mock issue exists");
            issue.state_id = state_id.to_string();
            return Json(json!({
                "data": {
                    "issueUpdate": {
                        "success": true
                    }
                }
            }));
        }

        if query.contains("issueUpdate(") && variables.get("title").is_some() {
            let issue_id = variables
                .get("id")
                .and_then(Value::as_str)
                .expect("issue update issue id");
            let mut issues = state.issues.lock().expect("mock issues lock");
            let issue = issues.get_mut(issue_id).expect("mock issue exists");
            issue.title = variables
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            issue.description = variables
                .get("description")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            return Json(json!({
                "data": {
                    "issueUpdate": {
                        "success": true,
                        "issue": {
                            "id": issue.id,
                            "identifier": issue.identifier,
                        }
                    }
                }
            }));
        }

        if query.contains("commentCreate(") {
            return Json(json!({
                "data": {
                    "commentCreate": {
                        "success": true
                    }
                }
            }));
        }

        if query.contains("issue(id:$id)") {
            let issue_id = variables
                .get("id")
                .and_then(Value::as_str)
                .expect("issue lookup id");
            let issues = state.issues.lock().expect("mock issues lock");
            let issue = issues.get(issue_id).expect("mock issue exists");
            return Json(json!({
                "data": {
                    "issue": {
                        "id": issue.id,
                        "identifier": issue.identifier,
                        "title": issue.title,
                        "description": issue.description,
                        "state": mock_linear_state_json(&issue.state_id),
                    }
                }
            }));
        }

        if query.contains("team(id:$teamId){states") {
            return Json(json!({
                "data": {
                    "team": {
                        "states": [
                            mock_linear_state_json(MOCK_LINEAR_STATE_READY),
                            mock_linear_state_json(MOCK_LINEAR_STATE_IN_PROGRESS),
                            mock_linear_state_json(MOCK_LINEAR_STATE_READY_FOR_RELEASE),
                            mock_linear_state_json(MOCK_LINEAR_STATE_DONE),
                        ]
                    }
                }
            }));
        }

        Json(json!({
            "errors": [{
                "message": format!("unsupported mock query: {query}"),
            }]
        }))
    }

    async fn response_json(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response bytes");
        let json = serde_json::from_slice::<Value>(&body).expect("json response");
        (status, json)
    }

    fn sync_scope_headers(secret: &str, workspace: &str, project: &str) -> HeaderMap {
        let token = sign_hs256_jwt(
            secret,
            json!({
                "sub": "svc-sync",
                "exp": 4_102_444_800u64,
                "scope": "code247:intentions:write code247:intentions:sync",
                "code247_projects": [format!("{workspace}/{project}")]
            }),
        );
        bearer_headers(&token)
    }

    async fn intake_single_intention(
        state: AppState,
        headers: HeaderMap,
        workspace: &str,
        project: &str,
        intention_id: &str,
    ) -> String {
        let response = super::post_intentions(
            State(state.clone()),
            headers,
            Json(super::IntentionIntakeRequest {
                manifest: super::IntentionManifest {
                    workspace: workspace.to_string(),
                    project: project.to_string(),
                    updated_at: "2026-03-07T08:00:00Z".to_string(),
                    intentions: vec![super::IntentionRecord {
                        id: intention_id.to_string(),
                        title: "State governance smoke".to_string(),
                        r#type: None,
                        scope: None,
                        priority: Some("medium".to_string()),
                        tasks: vec![],
                    }],
                },
                source: "/tmp/state-governance".to_string(),
                revision: Some("state-governance-test".to_string()),
                ci_target: None,
            }),
        )
        .await
        .into_response();

        let (status, body) = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let link = state
            .intention_link_store
            .lock()
            .expect("intention_link_store lock")
            .get_link(workspace, project, intention_id)
            .expect("intention link exists");
        link.linear_issue_id
    }

    async fn sync_intention(
        state: AppState,
        headers: HeaderMap,
        workspace: &str,
        project: &str,
        intention_id: &str,
        ci_url: Option<&str>,
        evidence: Vec<(&str, &str)>,
    ) -> (StatusCode, Value) {
        let response = super::post_intentions_sync(
            State(state),
            headers,
            Json(super::IntentionSyncRequest {
                workspace: workspace.to_string(),
                project: project.to_string(),
                results: vec![super::IntentionSyncResultInput {
                    intention_id: intention_id.to_string(),
                    status: "success".to_string(),
                    summary: Some("state-governance test".to_string()),
                    ci: ci_url.map(|url| super::IntentionSyncCiInput {
                        queue_id: Some("q-state".to_string()),
                        job: Some("job-state".to_string()),
                        url: Some(url.to_string()),
                    }),
                    evidence: evidence
                        .into_iter()
                        .map(|(label, url)| super::IntentionSyncEvidenceInput {
                            label: label.to_string(),
                            url: url.to_string(),
                        })
                        .collect(),
                    set_done_on_success: Some(true),
                }],
            }),
        )
        .await
        .into_response();

        response_json(response).await
    }

    #[tokio::test]
    async fn sync_http_moves_in_progress_to_ready_for_release() {
        let mock_linear = MockLinearServer::spawn().await;
        let mut state = test_app_state(false, Some("jwt-secret"));
        state.linear_api_key = Some("linear-test-token".to_string());
        state.linear_api_base_url = mock_linear.base_url.clone();

        let workspace = "workspace-a";
        let project = "project-x";
        let intention_id = "intent-rr";
        let headers = sync_scope_headers("jwt-secret", workspace, project);
        let linear_issue_id =
            intake_single_intention(state.clone(), headers.clone(), workspace, project, intention_id)
                .await;
        mock_linear.set_issue_state(&linear_issue_id, MOCK_LINEAR_STATE_IN_PROGRESS);

        let (status, body) = sync_intention(
            state,
            headers,
            workspace,
            project,
            intention_id,
            Some("https://ci.example/run/1"),
            vec![],
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["errors"], json!([]));
        assert_eq!(body["synced"][0]["intention_id"], intention_id);
        assert_eq!(body["synced"][0]["moved_to_ready_for_release"], true);
        assert_eq!(body["synced"][0]["moved_to_done"], false);
        assert_eq!(body["synced"][0]["target_state"], "Ready for Release");
        assert_eq!(
            mock_linear.issue_state_name(&linear_issue_id),
            "Ready for Release"
        );
    }

    #[tokio::test]
    async fn sync_http_blocks_in_progress_to_done_even_with_deploy_evidence() {
        let mock_linear = MockLinearServer::spawn().await;
        let mut state = test_app_state(false, Some("jwt-secret"));
        state.linear_api_key = Some("linear-test-token".to_string());
        state.linear_api_base_url = mock_linear.base_url.clone();

        let workspace = "workspace-a";
        let project = "project-x";
        let intention_id = "intent-block-done";
        let headers = sync_scope_headers("jwt-secret", workspace, project);
        let linear_issue_id =
            intake_single_intention(state.clone(), headers.clone(), workspace, project, intention_id)
                .await;
        mock_linear.set_issue_state(&linear_issue_id, MOCK_LINEAR_STATE_IN_PROGRESS);

        let (status, body) = sync_intention(
            state,
            headers,
            workspace,
            project,
            intention_id,
            Some("https://ci.example/run/2"),
            vec![("deploy", "https://deploy.example/release/2")],
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["synced"], json!([]));
        assert_eq!(body["errors"][0]["intention_id"], intention_id);
        assert_eq!(body["errors"][0]["code"], "INVALID_STATE_TRANSITION");
        assert_eq!(
            mock_linear.issue_state_name(&linear_issue_id),
            "In Progress (Code247)"
        );
    }

    #[tokio::test]
    async fn sync_http_moves_ready_for_release_to_done_with_deploy_evidence() {
        let mock_linear = MockLinearServer::spawn().await;
        let mut state = test_app_state(false, Some("jwt-secret"));
        state.linear_api_key = Some("linear-test-token".to_string());
        state.linear_api_base_url = mock_linear.base_url.clone();

        let workspace = "workspace-a";
        let project = "project-x";
        let intention_id = "intent-done";
        let headers = sync_scope_headers("jwt-secret", workspace, project);
        let linear_issue_id =
            intake_single_intention(state.clone(), headers.clone(), workspace, project, intention_id)
                .await;
        mock_linear.set_issue_state(&linear_issue_id, MOCK_LINEAR_STATE_READY_FOR_RELEASE);

        let (status, body) = sync_intention(
            state,
            headers,
            workspace,
            project,
            intention_id,
            Some("https://ci.example/run/3"),
            vec![("deploy", "https://deploy.example/release/3")],
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["errors"], json!([]));
        assert_eq!(body["synced"][0]["intention_id"], intention_id);
        assert_eq!(body["synced"][0]["moved_to_ready_for_release"], false);
        assert_eq!(body["synced"][0]["moved_to_done"], true);
        assert_eq!(body["synced"][0]["target_state"], "Done");
        assert_eq!(mock_linear.issue_state_name(&linear_issue_id), "Done");
    }
}
