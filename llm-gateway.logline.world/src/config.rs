//! Configuration for llm-gateway
//! Loaded from ~/.llm-gateway/config.toml or environment variables.

use serde::Deserialize;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_gateway_api_key")]
    pub api_key: String,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_local_routes")]
    pub routes: Vec<LlmRoute>,
    #[serde(default)]
    pub premium: PremiumProviders,
    #[serde(default)]
    pub model_matrix: ModelMatrix,
    #[serde(default)]
    pub reliability: ReliabilityPolicy,
    #[serde(default)]
    pub security: SecurityPolicy,
    #[serde(default)]
    pub qc: QcPolicy,
    #[serde(default)]
    pub supabase: SupabaseConfig,
    #[serde(default)]
    pub obs_api: ObsApiConfig,
    #[serde(default)]
    pub local: LocalPolicy,
}

fn default_port() -> u16 {
    3000
}

fn default_gateway_api_key() -> String {
    String::new()
}

fn resolve_gateway_api_key(current: String) -> String {
    if !current.trim().is_empty() && current.trim() != "lab-key-2024" {
        return current;
    }
    if let Ok(v) = std::env::var("LLM_API_KEY") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    panic!(
        "Missing secure gateway API key. Set LLM_API_KEY to a strong secret (>=32 random chars)."
    );
}

fn default_mode() -> String {
    "code".into()
}

fn canonical_mode(mode: &str) -> String {
    match mode.trim().to_ascii_lowercase().as_str() {
        "genius" | "premium" => "genius".into(),
        "fast" => "fast".into(),
        "code" | "auto" | "local" => "code".into(),
        _ => "code".into(),
    }
}

fn canonical_legacy_api_key_mode(mode: &str) -> String {
    match mode.trim().to_ascii_lowercase().as_str() {
        "disabled" | "jwt_only" | "off" => "disabled".into(),
        "legacy_only" | "legacy" => "legacy_only".into(),
        _ => "compat".into(),
    }
}

pub fn default_local_routes() -> Vec<LlmRoute> {
    vec![
        LlmRoute {
            name: "lab-8gb".into(),
            url: std::env::var("LAB_8GB_URL")
                .unwrap_or_else(|_| "http://192.168.0.199:11434".into()),
            model: std::env::var("LAB_8GB_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".into()),
            aliases: vec!["coder".into(), "code".into()],
            keep_alive: None,
            options: LocalOllamaOptions::default(),
        },
        LlmRoute {
            name: "lab-512".into(),
            url: std::env::var("LAB_512_URL").unwrap_or_else(|_| "http://localhost:11434".into()),
            model: std::env::var("LAB_512_MODEL").unwrap_or_else(|_| "qwen2.5:3b".into()),
            aliases: vec!["qwen".into(), "default".into(), "fast".into()],
            keep_alive: None,
            options: LocalOllamaOptions::default(),
        },
        LlmRoute {
            name: "lab-256".into(),
            url: std::env::var("LAB_256_URL")
                .unwrap_or_else(|_| "http://192.168.0.125:11434".into()),
            model: std::env::var("LAB_256_MODEL").unwrap_or_else(|_| "llama3.2:3b".into()),
            aliases: vec!["llama".into(), "meta".into()],
            keep_alive: None,
            options: LocalOllamaOptions::default(),
        },
    ]
}

#[derive(Clone, Deserialize)]
pub struct LlmRoute {
    pub name: String,
    pub url: String,
    pub model: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub keep_alive: Option<String>,
    #[serde(default)]
    pub options: LocalOllamaOptions,
}

#[derive(Clone, Deserialize, Default)]
pub struct LocalOllamaOptions {
    pub num_ctx: Option<u32>,
    pub num_batch: Option<u32>,
    pub num_thread: Option<u32>,
    pub num_gpu: Option<i32>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
}

impl LocalOllamaOptions {
    fn merged_with_override(&self, override_opts: &LocalOllamaOptions) -> LocalOllamaOptions {
        LocalOllamaOptions {
            num_ctx: override_opts.num_ctx.or(self.num_ctx),
            num_batch: override_opts.num_batch.or(self.num_batch),
            num_thread: override_opts.num_thread.or(self.num_thread),
            num_gpu: override_opts.num_gpu.or(self.num_gpu),
            top_k: override_opts.top_k.or(self.top_k),
            top_p: override_opts.top_p.or(self.top_p),
            repeat_penalty: override_opts.repeat_penalty.or(self.repeat_penalty),
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct LocalPolicy {
    #[serde(default = "default_local_max_inflight_requests")]
    pub max_inflight_requests: usize,
    #[serde(default = "default_local_max_queue_wait_ms")]
    pub max_queue_wait_ms: u64,
    #[serde(default = "default_local_keep_alive")]
    pub keep_alive: String,
    #[serde(default)]
    pub options: LocalOllamaOptions,
    #[serde(default = "default_local_warmup_enabled")]
    pub warmup_enabled: bool,
    #[serde(default = "default_local_warmup_interval_secs")]
    pub warmup_interval_secs: u64,
    #[serde(default = "default_local_warmup_timeout_ms")]
    pub warmup_timeout_ms: u64,
    #[serde(default = "default_local_adaptive_tuning_enabled")]
    pub adaptive_tuning_enabled: bool,
    #[serde(default = "default_local_adaptive_min_samples")]
    pub adaptive_min_samples: usize,
    #[serde(default = "default_local_adaptive_p95_degraded_ms")]
    pub adaptive_p95_degraded_ms: u64,
    #[serde(default = "default_local_adaptive_p99_emergency_ms")]
    pub adaptive_p99_emergency_ms: u64,
    #[serde(default = "default_local_adaptive_degraded_queue_wait_ms")]
    pub adaptive_degraded_queue_wait_ms: u64,
    #[serde(default = "default_local_adaptive_emergency_queue_wait_ms")]
    pub adaptive_emergency_queue_wait_ms: u64,
    #[serde(default = "default_local_adaptive_degraded_num_ctx_cap")]
    pub adaptive_degraded_num_ctx_cap: u32,
    #[serde(default = "default_local_adaptive_degraded_num_batch_cap")]
    pub adaptive_degraded_num_batch_cap: u32,
    #[serde(default = "default_local_adaptive_emergency_num_ctx_cap")]
    pub adaptive_emergency_num_ctx_cap: u32,
    #[serde(default = "default_local_adaptive_emergency_num_batch_cap")]
    pub adaptive_emergency_num_batch_cap: u32,
    #[serde(default = "default_local_default_max_tokens")]
    pub default_max_tokens: u32,
    #[serde(default = "default_local_adaptive_degraded_max_tokens")]
    pub adaptive_degraded_max_tokens: u32,
    #[serde(default = "default_local_adaptive_emergency_max_tokens")]
    pub adaptive_emergency_max_tokens: u32,
    #[serde(default = "default_local_energy_model_watts")]
    pub energy_model_watts: f64,
    #[serde(default = "default_local_energy_confidence_base")]
    pub energy_confidence_base: f64,
    #[serde(default = "default_local_energy_confidence_timing_bonus")]
    pub energy_confidence_timing_bonus: f64,
    #[serde(default = "default_local_energy_carbon_intensity_gco2e_per_kwh")]
    pub energy_carbon_intensity_gco2e_per_kwh: f64,
}

fn default_local_max_inflight_requests() -> usize {
    2
}

fn default_local_keep_alive() -> String {
    "30m".into()
}

fn default_local_max_queue_wait_ms() -> u64 {
    1200
}

fn default_local_warmup_enabled() -> bool {
    true
}

fn default_local_warmup_interval_secs() -> u64 {
    240
}

fn default_local_warmup_timeout_ms() -> u64 {
    5000
}

fn default_local_adaptive_tuning_enabled() -> bool {
    true
}

fn default_local_adaptive_min_samples() -> usize {
    30
}

fn default_local_adaptive_p95_degraded_ms() -> u64 {
    2500
}

fn default_local_adaptive_p99_emergency_ms() -> u64 {
    4500
}

fn default_local_adaptive_degraded_queue_wait_ms() -> u64 {
    900
}

fn default_local_adaptive_emergency_queue_wait_ms() -> u64 {
    450
}

fn default_local_adaptive_degraded_num_ctx_cap() -> u32 {
    4096
}

fn default_local_adaptive_degraded_num_batch_cap() -> u32 {
    256
}

fn default_local_adaptive_emergency_num_ctx_cap() -> u32 {
    2048
}

fn default_local_adaptive_emergency_num_batch_cap() -> u32 {
    128
}

fn default_local_default_max_tokens() -> u32 {
    768
}

fn default_local_adaptive_degraded_max_tokens() -> u32 {
    512
}

fn default_local_adaptive_emergency_max_tokens() -> u32 {
    384
}

fn default_local_energy_model_watts() -> f64 {
    220.0
}

fn default_local_energy_confidence_base() -> f64 {
    0.72
}

fn default_local_energy_confidence_timing_bonus() -> f64 {
    0.12
}

fn default_local_energy_carbon_intensity_gco2e_per_kwh() -> f64 {
    420.0
}

impl Default for LocalPolicy {
    fn default() -> Self {
        Self {
            max_inflight_requests: default_local_max_inflight_requests(),
            max_queue_wait_ms: default_local_max_queue_wait_ms(),
            keep_alive: default_local_keep_alive(),
            options: LocalOllamaOptions::default(),
            warmup_enabled: default_local_warmup_enabled(),
            warmup_interval_secs: default_local_warmup_interval_secs(),
            warmup_timeout_ms: default_local_warmup_timeout_ms(),
            adaptive_tuning_enabled: default_local_adaptive_tuning_enabled(),
            adaptive_min_samples: default_local_adaptive_min_samples(),
            adaptive_p95_degraded_ms: default_local_adaptive_p95_degraded_ms(),
            adaptive_p99_emergency_ms: default_local_adaptive_p99_emergency_ms(),
            adaptive_degraded_queue_wait_ms: default_local_adaptive_degraded_queue_wait_ms(),
            adaptive_emergency_queue_wait_ms: default_local_adaptive_emergency_queue_wait_ms(),
            adaptive_degraded_num_ctx_cap: default_local_adaptive_degraded_num_ctx_cap(),
            adaptive_degraded_num_batch_cap: default_local_adaptive_degraded_num_batch_cap(),
            adaptive_emergency_num_ctx_cap: default_local_adaptive_emergency_num_ctx_cap(),
            adaptive_emergency_num_batch_cap: default_local_adaptive_emergency_num_batch_cap(),
            default_max_tokens: default_local_default_max_tokens(),
            adaptive_degraded_max_tokens: default_local_adaptive_degraded_max_tokens(),
            adaptive_emergency_max_tokens: default_local_adaptive_emergency_max_tokens(),
            energy_model_watts: default_local_energy_model_watts(),
            energy_confidence_base: default_local_energy_confidence_base(),
            energy_confidence_timing_bonus: default_local_energy_confidence_timing_bonus(),
            energy_carbon_intensity_gco2e_per_kwh:
                default_local_energy_carbon_intensity_gco2e_per_kwh(),
        }
    }
}

#[derive(Clone)]
pub struct LocalRequestParams {
    pub keep_alive: Option<String>,
    pub options: LocalOllamaOptions,
}

#[derive(Clone, Deserialize, Default)]
pub struct PremiumProviders {
    #[serde(default)]
    pub openai: ProviderConfig,
    #[serde(default)]
    pub anthropic: ProviderConfig,
    #[serde(default)]
    pub gemini: ProviderConfig,
}

#[derive(Clone, Deserialize, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

/// Model matrix with three optimized modes: genius, fast, code
/// Each mode has a primary model and fallback order per provider.
#[derive(Clone, Deserialize)]
pub struct ModelMatrix {
    #[serde(default = "default_openai_models")]
    pub openai: ProviderModels,
    #[serde(default = "default_anthropic_models")]
    pub anthropic: ProviderModels,
    #[serde(default = "default_gemini_models")]
    pub gemini: ProviderModels,
}

#[derive(Clone, Deserialize)]
pub struct ProviderModels {
    /// Best reasoning, complex analysis (expensive)
    pub genius: String,
    /// Quick responses, simple queries (cheap)
    pub fast: String,
    /// Coding-optimized (balanced)
    pub code: String,
}

fn default_openai_models() -> ProviderModels {
    ProviderModels {
        genius: "gpt-5.2".into(),           // Best quality
        fast: "gpt-5.1-chat-latest".into(), // Cheapest
        code: "gpt-5.1-codex".into(),       // Code-optimized
    }
}

fn default_anthropic_models() -> ProviderModels {
    ProviderModels {
        genius: "claude-opus-4.6".into(), // Best reasoning
        fast: "claude-haiku-4.5".into(),  // Cheapest (5-10x)
        code: "claude-sonnet-4.6".into(), // Best for code
    }
}

fn default_gemini_models() -> ProviderModels {
    ProviderModels {
        genius: "gemini-3.1-pro".into(), // Best quality
        fast: "gemini-2.5-flash".into(), // Ultra cheap
        code: "gemini-3.1-pro".into(),   // Pro is good for code
    }
}

impl Default for ModelMatrix {
    fn default() -> Self {
        Self {
            openai: default_openai_models(),
            anthropic: default_anthropic_models(),
            gemini: default_gemini_models(),
        }
    }
}

impl ProviderConfig {
    pub fn resolved_api_key(&self) -> Option<String> {
        if let Some(v) = &self.api_key {
            if !v.trim().is_empty() {
                return Some(v.clone());
            }
        }
        if let Some(env_name) = &self.api_key_env {
            if let Ok(v) = std::env::var(env_name) {
                if !v.trim().is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Deserialize)]
pub struct ReliabilityPolicy {
    #[serde(default = "default_per_attempt_timeout_ms")]
    pub per_attempt_timeout_ms: u64,
    #[serde(default = "default_total_timeout_ms")]
    pub max_total_timeout_ms: u64,
    #[serde(default = "default_max_attempts_auto")]
    pub max_attempts_auto: usize,
    #[serde(default = "default_max_attempts_premium")]
    pub max_attempts_premium: usize,
    #[serde(default = "default_max_local_attempts_auto")]
    pub max_local_attempts_auto: usize,
    #[serde(default = "default_retry_max_retries")]
    pub retry_max_retries: usize,
    #[serde(default = "default_retry_backoff_base_ms")]
    pub retry_backoff_base_ms: u64,
    #[serde(default = "default_circuit_breaker_failure_threshold")]
    pub circuit_breaker_failure_threshold: u32,
    #[serde(default = "default_circuit_breaker_cooldown_secs")]
    pub circuit_breaker_cooldown_secs: u64,
}

fn default_per_attempt_timeout_ms() -> u64 {
    20_000
}

fn default_total_timeout_ms() -> u64 {
    60_000
}

fn default_max_attempts_auto() -> usize {
    12
}

fn default_max_attempts_premium() -> usize {
    4
}

fn default_max_local_attempts_auto() -> usize {
    10
}

fn default_retry_max_retries() -> usize {
    1
}

fn default_retry_backoff_base_ms() -> u64 {
    250
}

fn default_circuit_breaker_failure_threshold() -> u32 {
    3
}

fn default_circuit_breaker_cooldown_secs() -> u64 {
    45
}

impl Default for ReliabilityPolicy {
    fn default() -> Self {
        Self {
            per_attempt_timeout_ms: default_per_attempt_timeout_ms(),
            max_total_timeout_ms: default_total_timeout_ms(),
            max_attempts_auto: default_max_attempts_auto(),
            max_attempts_premium: default_max_attempts_premium(),
            max_local_attempts_auto: default_max_local_attempts_auto(),
            retry_max_retries: default_retry_max_retries(),
            retry_backoff_base_ms: default_retry_backoff_base_ms(),
            circuit_breaker_failure_threshold: default_circuit_breaker_failure_threshold(),
            circuit_breaker_cooldown_secs: default_circuit_breaker_cooldown_secs(),
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct SecurityPolicy {
    #[serde(default = "default_cors_allow_origins")]
    pub cors_allow_origins: Vec<String>,
    #[serde(default)]
    pub expose_upstream_url: bool,
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
    #[serde(default = "default_legacy_api_key_mode")]
    pub legacy_api_key_mode: String,
    pub legacy_api_key_sunset_at: Option<String>,
    pub onboarding_jwt_secret: Option<String>,
    pub onboarding_jwt_audience: Option<String>,
}

/// Supabase connection for fuel billing and request logging
#[derive(Clone, Deserialize)]
pub struct SupabaseConfig {
    /// Supabase project URL (e.g., https://xxx.supabase.co)
    pub url: Option<String>,
    /// service_role key (bypasses RLS for fuel_events insert)
    pub service_role_key: Option<String>,
    /// JWT secret to validate caller tokens (from Supabase dashboard)
    pub jwt_secret: Option<String>,
    /// Expected JWT audience (optional)
    pub jwt_audience: Option<String>,
    /// Required scope for service-to-service JWTs (optional hardening)
    pub required_service_scope: Option<String>,
    /// Treat Supabase as primary sink for fuel accounting.
    #[serde(default = "default_true")]
    pub fuel_primary_enabled: bool,
    /// Allow SQLite fallback writes when Supabase fuel emit fails.
    #[serde(default = "default_true")]
    pub sqlite_fallback_enabled: bool,
    /// Enable cloud settlement reconciliation (OpenAI/Anthropic admin APIs).
    #[serde(default = "default_true")]
    pub settlement_enabled: bool,
    /// OpenAI admin API key for org usage/cost reconciliation.
    pub settlement_openai_admin_key: Option<String>,
    /// Anthropic admin API key for org usage/cost reconciliation.
    pub settlement_anthropic_admin_key: Option<String>,
    /// OpenAI base URL (default: https://api.openai.com)
    pub settlement_openai_base_url: Option<String>,
    /// Anthropic base URL (default: https://api.anthropic.com)
    pub settlement_anthropic_base_url: Option<String>,
    /// Max events processed in one settlement run.
    #[serde(default = "default_settlement_max_events_per_run")]
    pub settlement_max_events_per_run: u32,
    /// Retries for provider/Supabase settlement calls.
    #[serde(default = "default_settlement_retry_max_retries")]
    pub settlement_retry_max_retries: usize,
    /// Backoff base for settlement retries.
    #[serde(default = "default_settlement_retry_backoff_base_ms")]
    pub settlement_retry_backoff_base_ms: u64,
}

impl Default for SupabaseConfig {
    fn default() -> Self {
        Self {
            url: None,
            service_role_key: None,
            jwt_secret: None,
            jwt_audience: None,
            required_service_scope: None,
            fuel_primary_enabled: true,
            sqlite_fallback_enabled: true,
            settlement_enabled: true,
            settlement_openai_admin_key: None,
            settlement_anthropic_admin_key: None,
            settlement_openai_base_url: None,
            settlement_anthropic_base_url: None,
            settlement_max_events_per_run: default_settlement_max_events_per_run(),
            settlement_retry_max_retries: default_settlement_retry_max_retries(),
            settlement_retry_backoff_base_ms: default_settlement_retry_backoff_base_ms(),
        }
    }
}

fn default_rate_limit_per_minute() -> u32 {
    120
}

fn default_legacy_api_key_mode() -> String {
    "compat".into()
}

fn default_settlement_max_events_per_run() -> u32 {
    500
}

fn default_settlement_retry_max_retries() -> usize {
    2
}

fn default_settlement_retry_backoff_base_ms() -> u64 {
    350
}

/// obs-api ingestion target (optional)
#[derive(Clone, Deserialize, Default)]
pub struct ObsApiConfig {
    /// Base URL (e.g. https://obs-api.logline.world)
    pub base_url: Option<String>,
    /// Bearer token with obs:ingest scope
    pub token: Option<String>,
}

fn default_cors_allow_origins() -> Vec<String> {
    vec![
        "http://localhost:3000".into(),
        "http://127.0.0.1:3000".into(),
    ]
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            cors_allow_origins: default_cors_allow_origins(),
            expose_upstream_url: false,
            rate_limit_per_minute: default_rate_limit_per_minute(),
            legacy_api_key_mode: default_legacy_api_key_mode(),
            legacy_api_key_sunset_at: None,
            onboarding_jwt_secret: None,
            onboarding_jwt_audience: None,
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct QcPolicy {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_qc_sampling_rate")]
    pub sampling_rate: f64,
    #[serde(default)]
    pub include_stream: bool,
    #[serde(default = "default_qc_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_qc_max_excerpt_chars")]
    pub max_excerpt_chars: usize,
}

fn default_qc_sampling_rate() -> f64 {
    0.2
}

fn default_qc_retention_days() -> u32 {
    30
}

fn default_qc_max_excerpt_chars() -> usize {
    1200
}

impl Default for QcPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_rate: default_qc_sampling_rate(),
            include_stream: false,
            retention_days: default_qc_retention_days(),
            max_excerpt_chars: default_qc_max_excerpt_chars(),
        }
    }
}

impl Config {
    pub fn local_params_for_route(&self, base_url: &str, model: &str) -> LocalRequestParams {
        let route = self
            .routes
            .iter()
            .find(|r| r.url == base_url && r.model == model)
            .or_else(|| {
                self.routes.iter().find(|r| {
                    r.url == base_url
                        && (r.model.eq_ignore_ascii_case(model)
                            || r.aliases.iter().any(|a| a.eq_ignore_ascii_case(model)))
                })
            });

        let keep_alive = route.and_then(|r| r.keep_alive.clone()).or_else(|| {
            let v = self.local.keep_alive.trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        });

        let options = route.map_or_else(
            || self.local.options.clone(),
            |r| self.local.options.merged_with_override(&r.options),
        );

        LocalRequestParams {
            keep_alive,
            options,
        }
    }

    pub fn load() -> Self {
        let config_path = dirs::home_dir()
            .map(|h| h.join(".llm-gateway/config.toml"))
            .unwrap_or_else(|| "config.toml".into());

        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            match toml::from_str::<Config>(&contents) {
                Ok(mut cfg) => {
                    if cfg.routes.is_empty() {
                        cfg.routes = default_local_routes();
                    }
                    cfg.default_mode = canonical_mode(&cfg.default_mode);
                    normalize_local_policy(&mut cfg.local);
                    normalize_security_policy(&mut cfg.security);
                    normalize_supabase_config(&mut cfg.supabase);
                    cfg.api_key = resolve_gateway_api_key(cfg.api_key);
                    info!(path = ?config_path, "Loaded config from file");
                    return cfg;
                }
                Err(e) => warn!(error = %e, "failed to parse config file; using env fallback"),
            }
        }

        info!("Using env fallback config");
        let openai_key = std::env::var("OPENAI_API_KEY").ok();
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let gemini_key = std::env::var("GEMINI_API_KEY").ok();
        let expose_upstream_url = std::env::var("LLM_EXPOSE_UPSTREAM_URL")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let cors_allow_origins = std::env::var("CORS_ALLOW_ORIGINS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(default_cors_allow_origins);
        let qc_enabled = std::env::var("QC_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let qc_sampling_rate = std::env::var("QC_SAMPLING_RATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v.clamp(0.0, 1.0))
            .unwrap_or_else(default_qc_sampling_rate);
        let qc_include_stream = std::env::var("QC_INCLUDE_STREAM")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        let qc_retention_days = std::env::var("QC_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(default_qc_retention_days);
        let qc_max_excerpt_chars = std::env::var("QC_MAX_EXCERPT_CHARS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(default_qc_max_excerpt_chars);
        let local_max_inflight_requests = std::env::var("LLM_LOCAL_MAX_INFLIGHT")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(default_local_max_inflight_requests)
            .max(1);
        let local_max_queue_wait_ms = std::env::var("LLM_LOCAL_MAX_QUEUE_WAIT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_local_max_queue_wait_ms)
            .max(50);
        let local_keep_alive =
            std::env::var("LLM_LOCAL_KEEP_ALIVE").unwrap_or_else(|_| default_local_keep_alive());
        let local_warmup_enabled = std::env::var("LLM_LOCAL_WARMUP_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or_else(default_local_warmup_enabled);
        let local_warmup_interval_secs = std::env::var("LLM_LOCAL_WARMUP_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_local_warmup_interval_secs)
            .max(15);
        let local_warmup_timeout_ms = std::env::var("LLM_LOCAL_WARMUP_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_local_warmup_timeout_ms)
            .max(1000);
        let local_adaptive_tuning_enabled = std::env::var("LLM_LOCAL_ADAPTIVE_TUNING_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or_else(default_local_adaptive_tuning_enabled);
        let local_adaptive_min_samples = std::env::var("LLM_LOCAL_ADAPTIVE_MIN_SAMPLES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(default_local_adaptive_min_samples)
            .max(1);
        let local_adaptive_p95_degraded_ms = std::env::var("LLM_LOCAL_ADAPTIVE_P95_DEGRADED_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_local_adaptive_p95_degraded_ms)
            .max(100);
        let local_adaptive_p99_emergency_ms = std::env::var("LLM_LOCAL_ADAPTIVE_P99_EMERGENCY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(default_local_adaptive_p99_emergency_ms)
            .max(local_adaptive_p95_degraded_ms);
        let local_adaptive_degraded_queue_wait_ms =
            std::env::var("LLM_LOCAL_ADAPTIVE_DEGRADED_QUEUE_WAIT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(default_local_adaptive_degraded_queue_wait_ms)
                .max(50);
        let local_adaptive_emergency_queue_wait_ms =
            std::env::var("LLM_LOCAL_ADAPTIVE_EMERGENCY_QUEUE_WAIT_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(default_local_adaptive_emergency_queue_wait_ms)
                .max(50)
                .min(local_adaptive_degraded_queue_wait_ms);
        let local_adaptive_degraded_num_ctx_cap =
            std::env::var("LLM_LOCAL_ADAPTIVE_DEGRADED_NUM_CTX_CAP")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_degraded_num_ctx_cap)
                .max(128);
        let local_adaptive_degraded_num_batch_cap =
            std::env::var("LLM_LOCAL_ADAPTIVE_DEGRADED_NUM_BATCH_CAP")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_degraded_num_batch_cap)
                .max(16);
        let local_adaptive_emergency_num_ctx_cap =
            std::env::var("LLM_LOCAL_ADAPTIVE_EMERGENCY_NUM_CTX_CAP")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_emergency_num_ctx_cap)
                .max(128)
                .min(local_adaptive_degraded_num_ctx_cap);
        let local_adaptive_emergency_num_batch_cap =
            std::env::var("LLM_LOCAL_ADAPTIVE_EMERGENCY_NUM_BATCH_CAP")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_emergency_num_batch_cap)
                .max(16)
                .min(local_adaptive_degraded_num_batch_cap);
        let local_options = LocalOllamaOptions {
            num_ctx: std::env::var("LLM_LOCAL_NUM_CTX")
                .ok()
                .and_then(|v| v.parse::<u32>().ok()),
            num_batch: std::env::var("LLM_LOCAL_NUM_BATCH")
                .ok()
                .and_then(|v| v.parse::<u32>().ok()),
            num_thread: std::env::var("LLM_LOCAL_NUM_THREAD")
                .ok()
                .and_then(|v| v.parse::<u32>().ok()),
            num_gpu: std::env::var("LLM_LOCAL_NUM_GPU")
                .ok()
                .and_then(|v| v.parse::<i32>().ok()),
            top_k: std::env::var("LLM_LOCAL_TOP_K")
                .ok()
                .and_then(|v| v.parse::<u32>().ok()),
            top_p: std::env::var("LLM_LOCAL_TOP_P")
                .ok()
                .and_then(|v| v.parse::<f32>().ok()),
            repeat_penalty: std::env::var("LLM_LOCAL_REPEAT_PENALTY")
                .ok()
                .and_then(|v| v.parse::<f32>().ok()),
        };
        let onboarding_jwt_secret = std::env::var("CLI_JWT_SECRET").ok();
        let onboarding_jwt_audience = std::env::var("CLI_JWT_AUDIENCE").ok();
        let rate_limit_per_minute = std::env::var("LLM_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(default_rate_limit_per_minute)
            .max(1);
        let legacy_api_key_mode = canonical_legacy_api_key_mode(
            &std::env::var("LLM_LEGACY_API_KEY_MODE")
                .unwrap_or_else(|_| default_legacy_api_key_mode()),
        );
        let legacy_api_key_sunset_at = std::env::var("LLM_LEGACY_API_KEY_SUNSET_AT").ok();
        let local_default_max_tokens = std::env::var("LLM_LOCAL_DEFAULT_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(default_local_default_max_tokens)
            .max(64);
        let local_adaptive_degraded_max_tokens =
            std::env::var("LLM_LOCAL_ADAPTIVE_DEGRADED_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_degraded_max_tokens)
                .max(64)
                .min(local_default_max_tokens);
        let local_adaptive_emergency_max_tokens =
            std::env::var("LLM_LOCAL_ADAPTIVE_EMERGENCY_MAX_TOKENS")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_local_adaptive_emergency_max_tokens)
                .max(64)
                .min(local_adaptive_degraded_max_tokens);
        let local_energy_model_watts = std::env::var("LLM_LOCAL_ENERGY_MODEL_WATTS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or_else(default_local_energy_model_watts)
            .max(1.0);
        let local_energy_confidence_base = std::env::var("LLM_LOCAL_ENERGY_CONFIDENCE_BASE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or_else(default_local_energy_confidence_base)
            .clamp(0.0, 1.0);
        let local_energy_confidence_timing_bonus =
            std::env::var("LLM_LOCAL_ENERGY_CONFIDENCE_TIMING_BONUS")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or_else(default_local_energy_confidence_timing_bonus)
                .clamp(0.0, 1.0);
        let local_energy_carbon_intensity_gco2e_per_kwh =
            std::env::var("LLM_LOCAL_ENERGY_CARBON_GCO2E_PER_KWH")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or_else(default_local_energy_carbon_intensity_gco2e_per_kwh)
                .max(0.0);
        let supabase_fuel_primary_enabled = std::env::var("SUPABASE_FUEL_PRIMARY_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let supabase_sqlite_fallback_enabled = std::env::var("SUPABASE_SQLITE_FALLBACK_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let supabase_settlement_enabled = std::env::var("SUPABASE_SETTLEMENT_ENABLED")
            .ok()
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let supabase_settlement_max_events_per_run =
            std::env::var("SUPABASE_SETTLEMENT_MAX_EVENTS_PER_RUN")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or_else(default_settlement_max_events_per_run)
                .max(1);
        let supabase_settlement_retry_max_retries =
            std::env::var("SUPABASE_SETTLEMENT_RETRY_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or_else(default_settlement_retry_max_retries);
        let supabase_settlement_retry_backoff_base_ms =
            std::env::var("SUPABASE_SETTLEMENT_RETRY_BACKOFF_BASE_MS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(default_settlement_retry_backoff_base_ms)
                .max(50);

        let mut cfg = Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            api_key: resolve_gateway_api_key(default_gateway_api_key()),
            default_mode: canonical_mode(
                &std::env::var("LLM_DEFAULT_MODE").unwrap_or_else(|_| default_mode()),
            ),
            routes: default_local_routes(),
            premium: PremiumProviders {
                openai: ProviderConfig {
                    enabled: openai_key.is_some(),
                    api_key: openai_key,
                    api_key_env: None,
                    base_url: Some("https://api.openai.com".into()),
                    default_model: Some("gpt-5.2".into()),
                },
                anthropic: ProviderConfig {
                    enabled: anthropic_key.is_some(),
                    api_key: anthropic_key,
                    api_key_env: None,
                    base_url: Some("https://api.anthropic.com".into()),
                    default_model: Some("claude-sonnet-4-6".into()),
                },
                gemini: ProviderConfig {
                    enabled: gemini_key.is_some(),
                    api_key: gemini_key,
                    api_key_env: None,
                    base_url: Some("https://generativelanguage.googleapis.com".into()),
                    default_model: Some("gemini-3.1-pro".into()),
                },
            },
            reliability: ReliabilityPolicy::default(),
            security: SecurityPolicy {
                cors_allow_origins,
                expose_upstream_url,
                rate_limit_per_minute,
                legacy_api_key_mode,
                legacy_api_key_sunset_at,
                onboarding_jwt_secret,
                onboarding_jwt_audience,
            },
            qc: QcPolicy {
                enabled: qc_enabled,
                sampling_rate: qc_sampling_rate,
                include_stream: qc_include_stream,
                retention_days: qc_retention_days,
                max_excerpt_chars: qc_max_excerpt_chars,
            },
            supabase: SupabaseConfig {
                url: std::env::var("SUPABASE_URL").ok(),
                service_role_key: std::env::var("SUPABASE_SERVICE_ROLE_KEY").ok(),
                jwt_secret: std::env::var("SUPABASE_JWT_SECRET").ok(),
                jwt_audience: std::env::var("SUPABASE_JWT_AUDIENCE").ok(),
                required_service_scope: std::env::var("SUPABASE_REQUIRED_SERVICE_SCOPE").ok(),
                fuel_primary_enabled: supabase_fuel_primary_enabled,
                sqlite_fallback_enabled: supabase_sqlite_fallback_enabled,
                settlement_enabled: supabase_settlement_enabled,
                settlement_openai_admin_key: std::env::var("OPENAI_ADMIN_API_KEY").ok(),
                settlement_anthropic_admin_key: std::env::var("ANTHROPIC_ADMIN_API_KEY").ok(),
                settlement_openai_base_url: std::env::var("OPENAI_SETTLEMENT_BASE_URL").ok(),
                settlement_anthropic_base_url: std::env::var("ANTHROPIC_SETTLEMENT_BASE_URL").ok(),
                settlement_max_events_per_run: supabase_settlement_max_events_per_run,
                settlement_retry_max_retries: supabase_settlement_retry_max_retries,
                settlement_retry_backoff_base_ms: supabase_settlement_retry_backoff_base_ms,
            },
            obs_api: ObsApiConfig {
                base_url: std::env::var("OBS_API_BASE_URL").ok(),
                token: std::env::var("OBS_API_TOKEN").ok(),
            },
            model_matrix: ModelMatrix::default(),
            local: LocalPolicy {
                max_inflight_requests: local_max_inflight_requests,
                max_queue_wait_ms: local_max_queue_wait_ms,
                keep_alive: local_keep_alive,
                options: local_options,
                warmup_enabled: local_warmup_enabled,
                warmup_interval_secs: local_warmup_interval_secs,
                warmup_timeout_ms: local_warmup_timeout_ms,
                adaptive_tuning_enabled: local_adaptive_tuning_enabled,
                adaptive_min_samples: local_adaptive_min_samples,
                adaptive_p95_degraded_ms: local_adaptive_p95_degraded_ms,
                adaptive_p99_emergency_ms: local_adaptive_p99_emergency_ms,
                adaptive_degraded_queue_wait_ms: local_adaptive_degraded_queue_wait_ms,
                adaptive_emergency_queue_wait_ms: local_adaptive_emergency_queue_wait_ms,
                adaptive_degraded_num_ctx_cap: local_adaptive_degraded_num_ctx_cap,
                adaptive_degraded_num_batch_cap: local_adaptive_degraded_num_batch_cap,
                adaptive_emergency_num_ctx_cap: local_adaptive_emergency_num_ctx_cap,
                adaptive_emergency_num_batch_cap: local_adaptive_emergency_num_batch_cap,
                default_max_tokens: local_default_max_tokens,
                adaptive_degraded_max_tokens: local_adaptive_degraded_max_tokens,
                adaptive_emergency_max_tokens: local_adaptive_emergency_max_tokens,
                energy_model_watts: local_energy_model_watts,
                energy_confidence_base: local_energy_confidence_base,
                energy_confidence_timing_bonus: local_energy_confidence_timing_bonus,
                energy_carbon_intensity_gco2e_per_kwh: local_energy_carbon_intensity_gco2e_per_kwh,
            },
        };
        normalize_local_policy(&mut cfg.local);
        normalize_security_policy(&mut cfg.security);
        normalize_supabase_config(&mut cfg.supabase);
        cfg
    }
}

fn normalize_local_policy(local: &mut LocalPolicy) {
    local.max_inflight_requests = local.max_inflight_requests.max(1);
    local.max_queue_wait_ms = local.max_queue_wait_ms.max(50);
    local.warmup_interval_secs = local.warmup_interval_secs.max(15);
    local.warmup_timeout_ms = local.warmup_timeout_ms.max(1000);
    local.adaptive_min_samples = local.adaptive_min_samples.max(1);
    local.adaptive_p95_degraded_ms = local.adaptive_p95_degraded_ms.max(100);
    local.adaptive_p99_emergency_ms = local
        .adaptive_p99_emergency_ms
        .max(local.adaptive_p95_degraded_ms);
    local.adaptive_degraded_queue_wait_ms = local
        .adaptive_degraded_queue_wait_ms
        .max(50)
        .min(local.max_queue_wait_ms);
    local.adaptive_emergency_queue_wait_ms = local
        .adaptive_emergency_queue_wait_ms
        .max(50)
        .min(local.adaptive_degraded_queue_wait_ms);
    local.adaptive_degraded_num_ctx_cap = local.adaptive_degraded_num_ctx_cap.max(128);
    local.adaptive_degraded_num_batch_cap = local.adaptive_degraded_num_batch_cap.max(16);
    local.adaptive_emergency_num_ctx_cap = local
        .adaptive_emergency_num_ctx_cap
        .max(128)
        .min(local.adaptive_degraded_num_ctx_cap);
    local.adaptive_emergency_num_batch_cap = local
        .adaptive_emergency_num_batch_cap
        .max(16)
        .min(local.adaptive_degraded_num_batch_cap);
    local.default_max_tokens = local.default_max_tokens.max(64);
    local.adaptive_degraded_max_tokens = local
        .adaptive_degraded_max_tokens
        .max(64)
        .min(local.default_max_tokens);
    local.adaptive_emergency_max_tokens = local
        .adaptive_emergency_max_tokens
        .max(64)
        .min(local.adaptive_degraded_max_tokens);
    local.energy_model_watts = local.energy_model_watts.max(1.0);
    local.energy_confidence_base = local.energy_confidence_base.clamp(0.0, 1.0);
    local.energy_confidence_timing_bonus = local.energy_confidence_timing_bonus.clamp(0.0, 1.0);
    local.energy_carbon_intensity_gco2e_per_kwh =
        local.energy_carbon_intensity_gco2e_per_kwh.max(0.0);
}

fn normalize_security_policy(security: &mut SecurityPolicy) {
    security.rate_limit_per_minute = security.rate_limit_per_minute.max(1);
    security.legacy_api_key_mode = canonical_legacy_api_key_mode(&security.legacy_api_key_mode);
}

fn normalize_supabase_config(supabase: &mut SupabaseConfig) {
    supabase.settlement_max_events_per_run = supabase.settlement_max_events_per_run.max(1);
    supabase.settlement_retry_backoff_base_ms = supabase.settlement_retry_backoff_base_ms.max(50);
}

pub fn maybe_redact_upstream_url(config: &Config, upstream_url: &str) -> String {
    if config.security.expose_upstream_url {
        upstream_url.to_string()
    } else {
        "redacted".into()
    }
}

pub fn default_fuel_db_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".llm-gateway").join("fuel.db"))
        .unwrap_or_else(|| PathBuf::from("fuel.db"))
}

/// Check if a provider is ready (enabled and has API key)
pub fn provider_ready(cfg: &ProviderConfig) -> bool {
    cfg.enabled && cfg.resolved_api_key().is_some()
}
