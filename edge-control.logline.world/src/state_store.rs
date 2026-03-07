use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
use reqwest::StatusCode;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::config::{Config, IdempotencyBackend};

#[derive(Clone)]
pub struct StateStore {
    backend: Arc<StateStoreBackend>,
}

enum StateStoreBackend {
    Sqlite(SqliteStateStore),
    Supabase(SupabaseStateStore),
}

struct SqliteStateStore {
    conn: Arc<Mutex<Connection>>,
}

struct SupabaseStateStore {
    rpc_base_url: String,
    service_role_key: String,
    owner_app_id: Option<String>,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ClaimIdempotencyRequest<'a> {
    p_key: &'a str,
    p_method: &'a str,
    p_path: &'a str,
    p_ttl_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    p_owner_app_id: Option<&'a str>,
}

#[derive(Serialize)]
struct ReleaseIdempotencyRequest<'a> {
    p_key: &'a str,
}

impl StateStore {
    pub fn from_config(config: &Config) -> Result<Self> {
        match config.idempotency_backend {
            IdempotencyBackend::Sqlite => Ok(Self {
                backend: Arc::new(StateStoreBackend::Sqlite(SqliteStateStore::open(
                    &config.state_db_path,
                )?)),
            }),
            IdempotencyBackend::Supabase => Ok(Self {
                backend: Arc::new(StateStoreBackend::Supabase(
                    SupabaseStateStore::from_config(config)?,
                )),
            }),
            IdempotencyBackend::Auto => {
                if config.supabase_url.is_some() && config.supabase_service_role_key.is_some() {
                    Ok(Self {
                        backend: Arc::new(StateStoreBackend::Supabase(
                            SupabaseStateStore::from_config(config)?,
                        )),
                    })
                } else {
                    Ok(Self {
                        backend: Arc::new(StateStoreBackend::Sqlite(SqliteStateStore::open(
                            &config.state_db_path,
                        )?)),
                    })
                }
            }
        }
    }

    pub async fn register_idempotency_key(
        &self,
        key: &str,
        method: &str,
        path: &str,
        ttl_seconds: u64,
    ) -> Result<bool> {
        match self.backend.as_ref() {
            StateStoreBackend::Sqlite(store) => {
                store.register_idempotency_key(key, method, path, ttl_seconds)
            }
            StateStoreBackend::Supabase(store) => {
                store
                    .register_idempotency_key(key, method, path, ttl_seconds)
                    .await
            }
        }
    }

    pub async fn remove_idempotency_key(&self, key: &str) -> Result<()> {
        match self.backend.as_ref() {
            StateStoreBackend::Sqlite(store) => store.remove_idempotency_key(key),
            StateStoreBackend::Supabase(store) => store.remove_idempotency_key(key).await,
        }
    }
}

impl SqliteStateStore {
    fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> Result<()> {
        self.conn.lock().expect("db lock").execute_batch(
            "
            CREATE TABLE IF NOT EXISTS idempotency_keys (
                key TEXT PRIMARY KEY,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    fn register_idempotency_key(
        &self,
        key: &str,
        method: &str,
        path: &str,
        ttl_seconds: u64,
    ) -> Result<bool> {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_seconds.max(60) as i64);
        let now_rfc3339 = now.to_rfc3339();
        let expires_at_rfc3339 = expires_at.to_rfc3339();
        let conn = self.conn.lock().expect("db lock");
        conn.execute(
            "DELETE FROM idempotency_keys WHERE expires_at <= ?",
            params![now_rfc3339],
        )?;
        let affected = conn.execute(
            "INSERT INTO idempotency_keys (key, method, path, first_seen_at, last_seen_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(key) DO NOTHING",
            params![key, method, path, now_rfc3339, now_rfc3339, expires_at_rfc3339],
        )?;
        Ok(affected > 0)
    }

    fn remove_idempotency_key(&self, key: &str) -> Result<()> {
        self.conn
            .lock()
            .expect("db lock")
            .execute("DELETE FROM idempotency_keys WHERE key = ?", params![key])?;
        Ok(())
    }
}

impl SupabaseStateStore {
    fn from_config(config: &Config) -> Result<Self> {
        let url = config
            .supabase_url
            .as_deref()
            .ok_or_else(|| anyhow!("SUPABASE_URL is required for supabase idempotency backend"))?;
        let service_role_key = config.supabase_service_role_key.as_deref().ok_or_else(|| {
            anyhow!("SUPABASE_SERVICE_ROLE_KEY is required for supabase idempotency backend")
        })?;

        Ok(Self {
            rpc_base_url: format!("{}/rest/v1/rpc", url.trim_end_matches('/')),
            service_role_key: service_role_key.to_string(),
            owner_app_id: config.default_app_id.clone(),
            client: reqwest::Client::new(),
        })
    }

    async fn register_idempotency_key(
        &self,
        key: &str,
        method: &str,
        path: &str,
        ttl_seconds: u64,
    ) -> Result<bool> {
        let request = ClaimIdempotencyRequest {
            p_key: key,
            p_method: method,
            p_path: path,
            p_ttl_seconds: ttl_seconds.max(60) as i64,
            p_owner_app_id: self.owner_app_id.as_deref(),
        };
        self.post_bool_rpc("edge_control_claim_idempotency_key", &request)
            .await
    }

    async fn remove_idempotency_key(&self, key: &str) -> Result<()> {
        let request = ReleaseIdempotencyRequest { p_key: key };
        let _ = self
            .post_bool_rpc("edge_control_release_idempotency_key", &request)
            .await?;
        Ok(())
    }

    async fn post_bool_rpc<T: Serialize>(&self, rpc_name: &str, body: &T) -> Result<bool> {
        let response = self
            .client
            .post(format!("{}/{}", self.rpc_base_url, rpc_name))
            .header("apikey", &self.service_role_key)
            .bearer_auth(&self.service_role_key)
            .json(body)
            .send()
            .await
            .with_context(|| format!("failed to call Supabase RPC {rpc_name}"))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(anyhow!(
                "Supabase RPC {rpc_name} not found; apply edge-control idempotency migration first"
            ));
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Supabase RPC {rpc_name} failed: {status} {body}"));
        }

        response
            .json::<bool>()
            .await
            .with_context(|| format!("invalid boolean response from Supabase RPC {rpc_name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteStateStore, SupabaseStateStore};
    use axum::{extract::State, routing::post, Json, Router};
    use serde::Deserialize;
    use std::{collections::HashSet, env, net::SocketAddr, sync::Arc};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[test]
    fn sqlite_store_rejects_duplicate_key_until_release() {
        let path = env::temp_dir().join(format!("edge-store-{}.db", Uuid::new_v4()));
        let store = SqliteStateStore::open(&path.display().to_string()).expect("store");

        assert!(store
            .register_idempotency_key("idem-a", "POST", "/v1/intention/draft", 300)
            .expect("first insert"));
        assert!(!store
            .register_idempotency_key("idem-a", "POST", "/v1/intention/draft", 300)
            .expect("duplicate insert"));

        store.remove_idempotency_key("idem-a").expect("release");

        assert!(store
            .register_idempotency_key("idem-a", "POST", "/v1/intention/draft", 300)
            .expect("reinsert after release"));
    }

    #[tokio::test]
    async fn supabase_store_uses_shared_rpc_contract() {
        #[derive(Deserialize)]
        struct ClaimPayload {
            p_key: String,
        }

        #[derive(Deserialize)]
        struct ReleasePayload {
            p_key: String,
        }

        async fn claim(
            State(keys): State<Arc<Mutex<HashSet<String>>>>,
            Json(payload): Json<ClaimPayload>,
        ) -> Json<bool> {
            let mut keys = keys.lock().await;
            Json(keys.insert(payload.p_key))
        }

        async fn release(
            State(keys): State<Arc<Mutex<HashSet<String>>>>,
            Json(payload): Json<ReleasePayload>,
        ) -> Json<bool> {
            let mut keys = keys.lock().await;
            Json(keys.remove(&payload.p_key))
        }

        let shared = Arc::new(Mutex::new(HashSet::<String>::new()));
        let app = Router::new()
            .route(
                "/rest/v1/rpc/edge_control_claim_idempotency_key",
                post(claim),
            )
            .route(
                "/rest/v1/rpc/edge_control_release_idempotency_key",
                post(release),
            )
            .with_state(shared);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr: SocketAddr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server");
        });

        let store = SupabaseStateStore {
            rpc_base_url: format!("http://{addr}/rest/v1/rpc"),
            service_role_key: "service-role-test".into(),
            owner_app_id: Some("edge-control-test".into()),
            client: reqwest::Client::new(),
        };

        assert!(store
            .register_idempotency_key("idem-b", "POST", "/v1/pr/risk", 300)
            .await
            .expect("first remote insert"));
        assert!(!store
            .register_idempotency_key("idem-b", "POST", "/v1/pr/risk", 300)
            .await
            .expect("duplicate remote insert"));

        store
            .remove_idempotency_key("idem-b")
            .await
            .expect("release");

        assert!(store
            .register_idempotency_key("idem-b", "POST", "/v1/pr/risk", 300)
            .await
            .expect("reinsert remote"));
    }
}
