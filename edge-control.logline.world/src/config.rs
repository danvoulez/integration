use anyhow::{anyhow, Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencyBackend {
    Auto,
    Sqlite,
    Supabase,
}

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub policy_set_path: String,
    pub supabase_url: Option<String>,
    pub supabase_service_role_key: Option<String>,
    pub supabase_jwt_secret: Option<String>,
    pub default_tenant_id: Option<String>,
    pub default_app_id: Option<String>,
    pub default_user_id: Option<String>,
    pub obs_api_base_url: Option<String>,
    pub obs_api_token: Option<String>,
    pub code247_base_url: String,
    pub code247_intentions_token: Option<String>,
    pub supabase_jwks_url: Option<String>,
    pub supabase_jwt_audience: Option<String>,
    pub internal_api_token: Option<String>,
    pub rate_limit_window_seconds: u64,
    pub rate_limit_max_requests: u32,
    pub rate_bucket_ttl_seconds: u64,
    pub rate_bucket_max_keys: usize,
    pub idempotency_ttl_seconds: u64,
    pub idempotency_backend: IdempotencyBackend,
    pub jwks_cache_ttl_seconds: u64,
    pub jwks_fetch_timeout_ms: u64,
    pub state_db_path: String,
    pub resilience_max_retries: u32,
    pub resilience_initial_backoff_ms: u64,
    pub resilience_circuit_failures: u32,
    pub resilience_circuit_open_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let host = std::env::var("EDGE_CONTROL_HOST").unwrap_or_else(|_| "0.0.0.0".into());
        let port = std::env::var("EDGE_CONTROL_PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse::<u16>()
            .context("EDGE_CONTROL_PORT must be a valid u16")?;

        let rate_limit_window_seconds = std::env::var("EDGE_CONTROL_RATE_LIMIT_WINDOW_SECONDS")
            .unwrap_or_else(|_| "60".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_RATE_LIMIT_WINDOW_SECONDS must be a valid u64")?;

        let rate_limit_max_requests = std::env::var("EDGE_CONTROL_RATE_LIMIT_MAX_REQUESTS")
            .unwrap_or_else(|_| "120".into())
            .parse::<u32>()
            .context("EDGE_CONTROL_RATE_LIMIT_MAX_REQUESTS must be a valid u32")?;
        let rate_bucket_ttl_seconds = std::env::var("EDGE_CONTROL_RATE_BUCKET_TTL_SECONDS")
            .unwrap_or_else(|_| "300".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_RATE_BUCKET_TTL_SECONDS must be a valid u64")?;
        let rate_bucket_max_keys = std::env::var("EDGE_CONTROL_RATE_BUCKET_MAX_KEYS")
            .unwrap_or_else(|_| "5000".into())
            .parse::<usize>()
            .context("EDGE_CONTROL_RATE_BUCKET_MAX_KEYS must be a valid usize")?;

        let idempotency_ttl_seconds = std::env::var("EDGE_CONTROL_IDEMPOTENCY_TTL_SECONDS")
            .unwrap_or_else(|_| "900".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_IDEMPOTENCY_TTL_SECONDS must be a valid u64")?;
        let idempotency_backend = match std::env::var("EDGE_CONTROL_IDEMPOTENCY_BACKEND")
            .unwrap_or_else(|_| "auto".into())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "auto" => IdempotencyBackend::Auto,
            "sqlite" => IdempotencyBackend::Sqlite,
            "supabase" => IdempotencyBackend::Supabase,
            other => {
                return Err(anyhow!(
                    "EDGE_CONTROL_IDEMPOTENCY_BACKEND must be one of: auto, sqlite, supabase (got {other})"
                ))
            }
        };
        let resilience_max_retries = std::env::var("EDGE_CONTROL_RESILIENCE_MAX_RETRIES")
            .unwrap_or_else(|_| "2".into())
            .parse::<u32>()
            .context("EDGE_CONTROL_RESILIENCE_MAX_RETRIES must be a valid u32")?;
        let resilience_initial_backoff_ms =
            std::env::var("EDGE_CONTROL_RESILIENCE_INITIAL_BACKOFF_MS")
                .unwrap_or_else(|_| "200".into())
                .parse::<u64>()
                .context("EDGE_CONTROL_RESILIENCE_INITIAL_BACKOFF_MS must be a valid u64")?;
        let resilience_circuit_failures = std::env::var("EDGE_CONTROL_CIRCUIT_FAILURES")
            .unwrap_or_else(|_| "3".into())
            .parse::<u32>()
            .context("EDGE_CONTROL_CIRCUIT_FAILURES must be a valid u32")?;
        let resilience_circuit_open_seconds = std::env::var("EDGE_CONTROL_CIRCUIT_OPEN_SECONDS")
            .unwrap_or_else(|_| "30".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_CIRCUIT_OPEN_SECONDS must be a valid u64")?;
        let supabase_url = std::env::var("SUPABASE_URL").ok();
        let supabase_jwks_url = std::env::var("SUPABASE_JWKS_URL").ok().or_else(|| {
            supabase_url.as_ref().map(|base| {
                format!(
                    "{}/auth/v1/.well-known/jwks.json",
                    base.trim_end_matches('/')
                )
            })
        });
        let jwks_cache_ttl_seconds = std::env::var("EDGE_CONTROL_JWKS_CACHE_TTL_SECONDS")
            .unwrap_or_else(|_| "300".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_JWKS_CACHE_TTL_SECONDS must be a valid u64")?;
        let jwks_fetch_timeout_ms = std::env::var("EDGE_CONTROL_JWKS_FETCH_TIMEOUT_MS")
            .unwrap_or_else(|_| "2000".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_JWKS_FETCH_TIMEOUT_MS must be a valid u64")?;

        Ok(Self {
            host,
            port,
            policy_set_path: std::env::var("EDGE_CONTROL_POLICY_SET_PATH")
                .unwrap_or_else(|_| "../policy/policy-set.v1.1.json".into()),
            supabase_url,
            supabase_service_role_key: std::env::var("SUPABASE_SERVICE_ROLE_KEY").ok(),
            supabase_jwt_secret: std::env::var("SUPABASE_JWT_SECRET").ok(),
            default_tenant_id: std::env::var("EDGE_CONTROL_DEFAULT_TENANT_ID").ok(),
            default_app_id: std::env::var("EDGE_CONTROL_DEFAULT_APP_ID").ok(),
            default_user_id: std::env::var("EDGE_CONTROL_DEFAULT_USER_ID").ok(),
            obs_api_base_url: std::env::var("OBS_API_BASE_URL").ok(),
            obs_api_token: std::env::var("OBS_API_TOKEN").ok(),
            code247_base_url: std::env::var("CODE247_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:4001".into()),
            code247_intentions_token: std::env::var("CODE247_INTENTIONS_TOKEN").ok(),
            supabase_jwks_url,
            supabase_jwt_audience: std::env::var("SUPABASE_JWT_AUDIENCE").ok(),
            internal_api_token: std::env::var("EDGE_CONTROL_INTERNAL_API_TOKEN").ok(),
            rate_limit_window_seconds,
            rate_limit_max_requests,
            rate_bucket_ttl_seconds,
            rate_bucket_max_keys,
            idempotency_ttl_seconds,
            idempotency_backend,
            jwks_cache_ttl_seconds,
            jwks_fetch_timeout_ms,
            state_db_path: std::env::var("EDGE_CONTROL_STATE_DB_PATH")
                .unwrap_or_else(|_| "edge-control.db".into()),
            resilience_max_retries,
            resilience_initial_backoff_ms,
            resilience_circuit_failures,
            resilience_circuit_open_seconds,
        })
    }

    pub fn auth_is_configured(&self) -> bool {
        self.supabase_jwks_url.is_some() || self.internal_api_token.is_some()
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
