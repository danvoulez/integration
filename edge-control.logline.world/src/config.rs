use anyhow::{Context, Result};

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub policy_set_path: String,
    pub supabase_url: Option<String>,
    pub supabase_service_role_key: Option<String>,
    pub default_tenant_id: Option<String>,
    pub default_app_id: Option<String>,
    pub default_user_id: Option<String>,
    pub obs_api_base_url: Option<String>,
    pub obs_api_token: Option<String>,
    pub code247_base_url: String,
    pub code247_intentions_token: Option<String>,
    pub supabase_jwt_secret: Option<String>,
    pub supabase_jwt_audience: Option<String>,
    pub internal_api_token: Option<String>,
    pub rate_limit_window_seconds: u64,
    pub rate_limit_max_requests: u32,
    pub idempotency_ttl_seconds: u64,
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

        let idempotency_ttl_seconds = std::env::var("EDGE_CONTROL_IDEMPOTENCY_TTL_SECONDS")
            .unwrap_or_else(|_| "900".into())
            .parse::<u64>()
            .context("EDGE_CONTROL_IDEMPOTENCY_TTL_SECONDS must be a valid u64")?;

        Ok(Self {
            host,
            port,
            policy_set_path: std::env::var("EDGE_CONTROL_POLICY_SET_PATH")
                .unwrap_or_else(|_| "../policy/policy-set.v1.1.json".into()),
            supabase_url: std::env::var("SUPABASE_URL").ok(),
            supabase_service_role_key: std::env::var("SUPABASE_SERVICE_ROLE_KEY").ok(),
            default_tenant_id: std::env::var("EDGE_CONTROL_DEFAULT_TENANT_ID").ok(),
            default_app_id: std::env::var("EDGE_CONTROL_DEFAULT_APP_ID").ok(),
            default_user_id: std::env::var("EDGE_CONTROL_DEFAULT_USER_ID").ok(),
            obs_api_base_url: std::env::var("OBS_API_BASE_URL").ok(),
            obs_api_token: std::env::var("OBS_API_TOKEN").ok(),
            code247_base_url: std::env::var("CODE247_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:4001".into()),
            code247_intentions_token: std::env::var("CODE247_INTENTIONS_TOKEN").ok(),
            supabase_jwt_secret: std::env::var("SUPABASE_JWT_SECRET").ok(),
            supabase_jwt_audience: std::env::var("SUPABASE_JWT_AUDIENCE").ok(),
            internal_api_token: std::env::var("EDGE_CONTROL_INTERNAL_API_TOKEN").ok(),
            rate_limit_window_seconds,
            rate_limit_max_requests,
            idempotency_ttl_seconds,
        })
    }

    pub fn auth_is_configured(&self) -> bool {
        self.supabase_jwt_secret.is_some() || self.internal_api_token.is_some()
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
