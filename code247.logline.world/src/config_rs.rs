use std::env;

use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct Config {
    pub db_path: String,
    pub evidence_path: String,
    pub repo_root: String,
    pub git_branch: String,
    pub git_remote: String,
    // LLM Gateway (primary - all LLM calls go through here)
    pub llm_gateway_url: String,
    pub llm_gateway_api_key: String,
    // Legacy direct adapters (deprecated, kept for fallback)
    pub anthropic_model: String,
    pub anthropic_api_key: Option<String>,
    pub ollama_model: String,
    pub ollama_base_url: String,
    // Linear
    pub linear_api_key: Option<String>,
    pub linear_api_base_url: String,
    pub linear_oauth_base_url: String,
    pub linear_team_id: String,
    pub linear_done_state_type: String,
    pub linear_client_id: Option<String>,
    pub linear_client_secret: Option<String>,
    pub linear_oauth_redirect_uri: Option<String>,
    pub linear_oauth_scopes: String,
    pub linear_oauth_actor: String,
    pub linear_oauth_state_ttl_seconds: i64,
    pub linear_oauth_refresh_lead_seconds: i64,
    pub linear_oauth_refresh_interval_seconds: u64,
    pub linear_claim_enabled: bool,
    pub linear_claim_state_name: String,
    pub linear_claim_in_progress_state_name: String,
    pub linear_ready_for_release_state_name: String,
    pub linear_claim_interval_seconds: u64,
    pub linear_claim_max_per_cycle: usize,
    pub linear_webhook_secret: Option<String>,
    pub linear_webhook_max_skew_seconds: i64,
    pub linear_webhook_poll_interval_seconds: u64,
    pub linear_webhook_retry_delay_seconds: u64,
    pub linear_webhook_max_attempts: i32,
    pub code247_public_url: String,
    pub code247_intentions_token: Option<String>,
    pub code247_auth_allow_legacy_token: bool,
    pub supabase_jwt_secret: Option<String>,
    pub supabase_jwt_secret_legacy: Option<String>,
    pub supabase_jwt_audience: Option<String>,
    pub code247_scope_jobs_read: String,
    pub code247_scope_jobs_write: String,
    pub code247_scope_intentions_write: String,
    pub code247_scope_intentions_sync: String,
    pub code247_scope_intentions_read: String,
    pub code247_scope_admin: String,
    pub code247_linear_meta_path: String,
    pub code247_supabase_sync_enabled: bool,
    pub code247_supabase_realtime_enabled: bool,
    pub code247_supabase_realtime_channel: String,
    pub supabase_url: Option<String>,
    pub supabase_service_role_key: Option<String>,
    pub supabase_tenant_id: Option<String>,
    pub supabase_app_id: Option<String>,
    pub supabase_user_id: Option<String>,
    pub obs_api_base_url: Option<String>,
    pub obs_api_token: Option<String>,
    // Server
    pub health_port: u16,
    pub poll_interval_ms: u64,
    pub max_review_iterations: u8,
    pub max_concurrent_jobs: usize,
    pub stage_lease_owner: String,
    pub stage_lease_sweep_interval_seconds: u64,
    pub stage_timeout_planning_seconds: i64,
    pub stage_timeout_coding_seconds: i64,
    pub stage_timeout_reviewing_seconds: i64,
    pub stage_timeout_validating_seconds: i64,
    pub stage_timeout_committing_seconds: i64,
    pub ci_flaky_reruns: u8,
    pub red_main_enforced: bool,
    pub red_main_flag_path: String,
    pub code247_runner_allowlist_enabled: bool,
    pub code247_runner_allowlist_manifest_path: String,
    pub project_manifest_path: String,
    pub project_manifest_schema_path: String,
    pub project_manifest_required: bool,
    // Project
    pub voulezvous_spec_path: String,
    pub github_token: Option<String>,
    pub github_repo: Option<String>,
    pub github_auto_merge_enabled: bool,
    pub github_auto_merge_timeout_seconds: u64,
    pub github_auto_merge_poll_seconds: u64,
    pub policy_set_path: String,
    pub policy_set_required: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let project_manifest_path = env::var("CODE247_MANIFEST_PATH")
            .unwrap_or_else(|_| ".code247/workspace.manifest.json".to_string());
        let runner_allowlist_manifest_path = env::var("CODE247_RUNNER_ALLOWLIST_MANIFEST_PATH")
            .unwrap_or_else(|_| project_manifest_path.clone());
        Ok(Self {
            db_path: env::var("DB_PATH").unwrap_or_else(|_| "dual_agents.db".to_string()),
            evidence_path: env::var("EVIDENCE_PATH").unwrap_or_else(|_| "evidence".to_string()),
            repo_root: env::var("REPO_ROOT").unwrap_or_else(|_| ".".to_string()),
            git_branch: env::var("GIT_BRANCH").unwrap_or_else(|_| "main".to_string()),
            git_remote: env::var("GIT_REMOTE").unwrap_or_else(|_| "origin".to_string()),
            // LLM Gateway (required - all LLM calls go through here)
            llm_gateway_url: env::var("LLM_GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:7700".to_string()),
            llm_gateway_api_key: required("LLM_GATEWAY_API_KEY")?,
            // Legacy direct adapters (deprecated)
            anthropic_model: env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string()),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY").ok(),
            ollama_model: env::var("OLLAMA_MODEL").unwrap_or_else(|_| "codellama".to_string()),
            ollama_base_url: env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            // Linear
            linear_api_key: env::var("LINEAR_API_KEY").ok(),
            linear_api_base_url: env::var("LINEAR_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.linear.app".to_string()),
            linear_oauth_base_url: env::var("LINEAR_OAUTH_BASE_URL")
                .unwrap_or_else(|_| "https://linear.app".to_string()),
            linear_team_id: required("LINEAR_TEAM_ID")?,
            linear_done_state_type: env::var("LINEAR_DONE_STATE_TYPE")
                .unwrap_or_else(|_| "completed".to_string()),
            linear_client_id: env::var("LINEAR_CLIENT_ID").ok(),
            linear_client_secret: env::var("LINEAR_CLIENT_SECRET").ok(),
            linear_oauth_redirect_uri: env::var("LINEAR_OAUTH_REDIRECT_URI").ok(),
            linear_oauth_scopes: env::var("LINEAR_OAUTH_SCOPES")
                .unwrap_or_else(|_| "read write comments:create issues:create".to_string()),
            linear_oauth_actor: env::var("LINEAR_OAUTH_ACTOR")
                .unwrap_or_else(|_| "app".to_string()),
            linear_oauth_state_ttl_seconds: parse_env("LINEAR_OAUTH_STATE_TTL_SECONDS", 600i64)?,
            linear_oauth_refresh_lead_seconds: parse_env(
                "LINEAR_OAUTH_REFRESH_LEAD_SECONDS",
                300i64,
            )?,
            linear_oauth_refresh_interval_seconds: parse_env(
                "LINEAR_OAUTH_REFRESH_INTERVAL_SECONDS",
                60u64,
            )?,
            linear_claim_enabled: parse_env_bool("LINEAR_CLAIM_ENABLED", true)?,
            linear_claim_state_name: env::var("LINEAR_CLAIM_STATE_NAME")
                .unwrap_or_else(|_| "Ready".to_string()),
            linear_claim_in_progress_state_name: env::var("LINEAR_CLAIM_IN_PROGRESS_STATE_NAME")
                .unwrap_or_else(|_| "In Progress".to_string()),
            linear_ready_for_release_state_name: env::var("LINEAR_READY_FOR_RELEASE_STATE_NAME")
                .unwrap_or_else(|_| "Ready for Release".to_string()),
            linear_claim_interval_seconds: parse_env("LINEAR_CLAIM_INTERVAL_SECONDS", 20u64)?,
            linear_claim_max_per_cycle: parse_env("LINEAR_CLAIM_MAX_PER_CYCLE", 25usize)?,
            linear_webhook_secret: env::var("LINEAR_WEBHOOK_SECRET")
                .ok()
                .or_else(|| env::var("LINEAR_WEBHOOK_SIGNING_SECRET").ok()),
            linear_webhook_max_skew_seconds: parse_env("LINEAR_WEBHOOK_MAX_SKEW_SECONDS", 60i64)?,
            linear_webhook_poll_interval_seconds: parse_env(
                "LINEAR_WEBHOOK_POLL_INTERVAL_SECONDS",
                5u64,
            )?,
            linear_webhook_retry_delay_seconds: parse_env(
                "LINEAR_WEBHOOK_RETRY_DELAY_SECONDS",
                60u64,
            )?,
            linear_webhook_max_attempts: parse_env("LINEAR_WEBHOOK_MAX_ATTEMPTS", 3i32)?,
            code247_public_url: env::var("CODE247_PUBLIC_URL")
                .unwrap_or_else(|_| "https://code247.logline.world".to_string()),
            code247_intentions_token: env::var("CODE247_INTENTIONS_TOKEN").ok(),
            code247_auth_allow_legacy_token: parse_env_bool(
                "CODE247_AUTH_ALLOW_LEGACY_TOKEN",
                false,
            )?,
            supabase_jwt_secret: env::var("SUPABASE_JWT_SECRET").ok(),
            supabase_jwt_secret_legacy: env::var("SUPABASE_JWT_SECRET_LEGACY")
                .ok()
                .or_else(|| env::var("SUPABASE_JWT_OLD_SECRET").ok()),
            supabase_jwt_audience: env::var("SUPABASE_JWT_AUDIENCE").ok(),
            code247_scope_jobs_read: env::var("CODE247_SCOPE_JOBS_READ")
                .unwrap_or_else(|_| "code247:jobs:read".to_string()),
            code247_scope_jobs_write: env::var("CODE247_SCOPE_JOBS_WRITE")
                .unwrap_or_else(|_| "code247:jobs:write".to_string()),
            code247_scope_intentions_write: env::var("CODE247_SCOPE_INTENTIONS_WRITE")
                .unwrap_or_else(|_| "code247:intentions:write".to_string()),
            code247_scope_intentions_sync: env::var("CODE247_SCOPE_INTENTIONS_SYNC")
                .unwrap_or_else(|_| "code247:intentions:sync".to_string()),
            code247_scope_intentions_read: env::var("CODE247_SCOPE_INTENTIONS_READ")
                .unwrap_or_else(|_| "code247:intentions:read".to_string()),
            code247_scope_admin: env::var("CODE247_SCOPE_ADMIN")
                .unwrap_or_else(|_| "code247:admin".to_string()),
            code247_linear_meta_path: env::var("CODE247_LINEAR_META_PATH")
                .unwrap_or_else(|_| ".code247/linear-meta.json".to_string()),
            code247_supabase_sync_enabled: parse_env_bool("CODE247_SUPABASE_SYNC_ENABLED", true)?,
            code247_supabase_realtime_enabled: parse_env_bool(
                "CODE247_SUPABASE_REALTIME_ENABLED",
                true,
            )?,
            code247_supabase_realtime_channel: env::var("CODE247_SUPABASE_REALTIME_CHANNEL")
                .unwrap_or_else(|_| "code247:jobs:{tenant_id}".to_string()),
            supabase_url: env::var("SUPABASE_URL").ok(),
            supabase_service_role_key: env::var("SUPABASE_SERVICE_ROLE_KEY")
                .ok()
                .or_else(|| env::var("SUPABASE_SERVICE_KEY").ok()),
            supabase_tenant_id: env::var("CODE247_SUPABASE_TENANT_ID").ok(),
            supabase_app_id: env::var("CODE247_SUPABASE_APP_ID").ok(),
            supabase_user_id: env::var("CODE247_SUPABASE_USER_ID").ok(),
            obs_api_base_url: env::var("OBS_API_BASE_URL").ok(),
            obs_api_token: env::var("OBS_API_TOKEN").ok(),
            // Server
            health_port: parse_env("HEALTH_PORT", 4001u16)?,
            poll_interval_ms: parse_env("POLL_INTERVAL_MS", 1000u64)?,
            max_review_iterations: parse_env("MAX_REVIEW_ITERATIONS", 2u8)?,
            max_concurrent_jobs: parse_env("MAX_CONCURRENT_JOBS", 3usize)?,
            stage_lease_owner: env::var("CODE247_STAGE_LEASE_OWNER")
                .unwrap_or_else(|_| format!("code247-{}", uuid::Uuid::new_v4())),
            stage_lease_sweep_interval_seconds: parse_env(
                "CODE247_STAGE_LEASE_SWEEP_INTERVAL_SECONDS",
                30u64,
            )?,
            stage_timeout_planning_seconds: parse_env(
                "CODE247_STAGE_TIMEOUT_PLANNING_SECONDS",
                900i64,
            )?,
            stage_timeout_coding_seconds: parse_env(
                "CODE247_STAGE_TIMEOUT_CODING_SECONDS",
                1800i64,
            )?,
            stage_timeout_reviewing_seconds: parse_env(
                "CODE247_STAGE_TIMEOUT_REVIEWING_SECONDS",
                900i64,
            )?,
            stage_timeout_validating_seconds: parse_env(
                "CODE247_STAGE_TIMEOUT_VALIDATING_SECONDS",
                1200i64,
            )?,
            stage_timeout_committing_seconds: parse_env(
                "CODE247_STAGE_TIMEOUT_COMMITTING_SECONDS",
                2100i64,
            )?,
            ci_flaky_reruns: parse_env("CODE247_CI_FLAKY_RERUNS", 1u8)?,
            red_main_enforced: parse_env_bool("CODE247_RED_MAIN_ENFORCED", true)?,
            red_main_flag_path: env::var("CODE247_RED_MAIN_FLAG_PATH")
                .unwrap_or_else(|_| ".code247/red-main.flag".to_string()),
            code247_runner_allowlist_enabled: parse_env_bool(
                "CODE247_RUNNER_ALLOWLIST_ENABLED",
                true,
            )?,
            code247_runner_allowlist_manifest_path: runner_allowlist_manifest_path,
            project_manifest_path,
            project_manifest_schema_path: env::var("CODE247_MANIFEST_SCHEMA_PATH")
                .unwrap_or_else(|_| "schemas/workspace.manifest.schema.json".to_string()),
            project_manifest_required: parse_env_bool("CODE247_MANIFEST_REQUIRED", false)?,
            // Project
            voulezvous_spec_path: env::var("VOULEZVOUS_SPEC_PATH")
                .unwrap_or_else(|_| "../voulezvous/docs/PLATAFORMA_SPEC.md".to_string()),
            github_token: env::var("GITHUB_TOKEN").ok(),
            github_repo: env::var("GITHUB_REPO").ok(),
            github_auto_merge_enabled: parse_env_bool("GITHUB_AUTO_MERGE_ENABLED", true)?,
            github_auto_merge_timeout_seconds: parse_env(
                "GITHUB_AUTO_MERGE_TIMEOUT_SECONDS",
                1800u64,
            )?,
            github_auto_merge_poll_seconds: parse_env("GITHUB_AUTO_MERGE_POLL_SECONDS", 20u64)?,
            policy_set_path: env::var("CODE247_POLICY_SET_PATH")
                .unwrap_or_else(|_| "../policy/policy-set.v1.1.json".to_string()),
            policy_set_required: parse_env_bool("CODE247_POLICY_SET_REQUIRED", true)?,
        })
    }

    pub fn linear_oauth_enabled(&self) -> bool {
        self.linear_client_id.is_some()
            && self.linear_client_secret.is_some()
            && self.linear_oauth_redirect_uri.is_some()
    }
}

fn parse_env_bool(key: &str, default: bool) -> Result<bool> {
    match env::var(key) {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => Err(anyhow!(
                    "invalid value for {key}: {raw} (expected true/false)"
                )),
            }
        }
        Err(_) => Ok(default),
    }
}

fn required(key: &str) -> Result<String> {
    env::var(key).map_err(|_| anyhow!("variável obrigatória ausente: {key}"))
}

fn parse_env<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(raw) => raw
            .parse::<T>()
            .map_err(|err| anyhow!("invalid value for {key}: {raw} ({err})")),
        Err(_) => Ok(default),
    }
}
