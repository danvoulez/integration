//! Supabase client configuration and initialization.

use crate::{Error, Result};
use reqwest::{Client, header};

/// Configuration for the Supabase client.
#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    /// Supabase project URL (e.g., `https://xxx.supabase.co`).
    pub url: String,
    /// Anonymous key for public access.
    pub anon_key: String,
    /// Optional service role key for elevated access.
    pub service_key: Option<String>,
}

impl SupabaseConfig {
    /// Create config from environment variables.
    ///
    /// Reads:
    /// - `SUPABASE_URL` (required)
    /// - `SUPABASE_ANON_KEY` (required)
    /// - `SUPABASE_SERVICE_KEY` (optional)
    pub fn from_env() -> Result<Self> {
        let url = std::env::var("SUPABASE_URL")
            .map_err(|_| Error::Config("SUPABASE_URL not set".into()))?;
        let anon_key = std::env::var("SUPABASE_ANON_KEY")
            .map_err(|_| Error::Config("SUPABASE_ANON_KEY not set".into()))?;
        let service_key = std::env::var("SUPABASE_SERVICE_KEY").ok();

        Ok(Self {
            url,
            anon_key,
            service_key,
        })
    }
}

/// Unified Supabase client for all ecosystem services.
///
/// # CLI-First Design
///
/// This client is designed to be consumed by the CLI first, then by other services.
/// All public methods should be usable from a CLI context (async runtime, no server state).
///
/// # Example (CLI)
///
/// ```ignore
/// let client = SupabaseClient::from_env()?;
/// client.set_jwt(user_jwt);
///
/// // Emit fuel event
/// client.emit_fuel(FuelEvent { ... }).await?;
///
/// // Query fuel
/// let events = client.query_fuel(FuelFilter::for_tenant("tenant-123")).await?;
/// ```
#[derive(Clone)]
pub struct SupabaseClient {
    config: SupabaseConfig,
    http: Client,
    /// Current JWT for authenticated requests (user or service).
    jwt: Option<String>,
}

impl SupabaseClient {
    /// Create a new client with the given configuration.
    pub fn new(config: SupabaseConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(Error::Http)?;

        Ok(Self {
            config,
            http,
            jwt: None,
        })
    }

    /// Create a client from environment variables.
    ///
    /// This is the primary constructor for CLI usage.
    pub fn from_env() -> Result<Self> {
        let config = SupabaseConfig::from_env()?;
        Self::new(config)
    }

    /// Set the JWT for authenticated requests.
    ///
    /// For CLI: This is the user's JWT from `~/.config/logline/auth.json`.
    /// For services: This is the service account JWT or service_key.
    pub fn set_jwt(&mut self, jwt: impl Into<String>) {
        self.jwt = Some(jwt.into());
    }

    /// Use service role key for elevated access (bypasses RLS).
    ///
    /// **Warning**: Only use for admin operations like bootstrap.
    pub fn use_service_role(&mut self) -> Result<()> {
        let key = self
            .config
            .service_key
            .clone()
            .ok_or_else(|| Error::Config("SUPABASE_SERVICE_KEY not set".into()))?;
        self.jwt = Some(key);
        Ok(())
    }

    /// Get the base URL for the Supabase project.
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get the PostgREST URL.
    pub fn postgrest_url(&self) -> String {
        format!("{}/rest/v1", self.config.url)
    }

    /// Get the Storage URL.
    pub fn storage_url(&self) -> String {
        format!("{}/storage/v1", self.config.url)
    }

    /// Get the Realtime URL.
    pub fn realtime_url(&self) -> String {
        format!("{}/realtime/v1", self.config.url)
    }

    /// Build headers for an authenticated request.
    pub(crate) fn auth_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();

        // API key header (always required)
        headers.insert(
            "apikey",
            self.config.anon_key.parse().expect("valid anon key"),
        );

        // Authorization header (JWT if set, otherwise anon key)
        let auth_value = self.jwt.as_ref().unwrap_or(&self.config.anon_key);
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", auth_value)
                .parse()
                .expect("valid auth header"),
        );

        // Prefer header for PostgREST
        headers.insert("Prefer", "return=representation".parse().unwrap());

        headers
    }

    /// Get the underlying HTTP client.
    pub(crate) fn http(&self) -> &Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        // This test requires env vars to be set
        std::env::set_var("SUPABASE_URL", "https://test.supabase.co");
        std::env::set_var("SUPABASE_ANON_KEY", "test-anon-key");

        let config = SupabaseConfig::from_env().unwrap();
        assert_eq!(config.url, "https://test.supabase.co");
        assert_eq!(config.anon_key, "test-anon-key");
        assert!(config.service_key.is_none());
    }
}
