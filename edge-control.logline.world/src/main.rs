mod auth;
mod config;
mod fuel;
mod handlers;
mod middleware;
mod models;
mod orchestration;
mod policy;
mod resilience;
mod state_store;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
    Router,
};
use tokio::sync::Mutex;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    config::Config, middleware::RateBucket, policy::PolicySet, resilience::CircuitBreakers,
    state_store::StateStore,
};

pub struct AppState {
    pub config: Config,
    pub policy_set: PolicySet,
    pub http_client: reqwest::Client,
    pub circuit_breakers: Arc<CircuitBreakers>,
    pub state_store: StateStore,
    rate_buckets: Mutex<HashMap<String, RateBucket>>,
}

impl AppState {
    pub(crate) fn new(config: Config, policy_set: PolicySet, state_store: StateStore) -> Self {
        Self {
            config,
            policy_set,
            http_client: reqwest::Client::new(),
            circuit_breakers: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            state_store,
            rate_buckets: Mutex::new(HashMap::new()),
        }
    }

    pub async fn consume_rate_slot(&self, key: &str) -> bool {
        let mut buckets = self.rate_buckets.lock().await;
        let entry = buckets
            .entry(key.to_string())
            .or_insert_with(RateBucket::new);
        entry.consume(
            Instant::now(),
            Duration::from_secs(self.config.rate_limit_window_seconds),
            self.config.rate_limit_max_requests,
        )
    }

    pub async fn register_idempotency_key(&self, key: &str, method: &str, path: &str) -> bool {
        self.state_store
            .register_idempotency_key(key, method, path, self.config.idempotency_ttl_seconds)
            .await
            .unwrap_or(false)
    }

    pub async fn remove_idempotency_key(&self, key: &str) {
        let _ = self.state_store.remove_idempotency_key(key).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;
    let policy_path = policy::resolve_policy_set_path(&config.policy_set_path);
    let policy_set = PolicySet::load(&policy_path)?;
    info!(policy_path=%policy_path, policy_version=%policy_set.version, "loaded policy set");
    let state_store = StateStore::from_config(&config)?;

    let state = Arc::new(AppState::new(config, policy_set, state_store));
    let app = build_app(state.clone());

    let listener = tokio::net::TcpListener::bind(state.config.bind_addr()).await?;
    info!(bind_addr=%state.config.bind_addr(), "edge-control listening");

    axum::serve(listener, app).await?;
    Ok(())
}

fn build_app(state: Arc<AppState>) -> Router {
    let protected_v1 = Router::new()
        .route("/intention/draft", post(handlers::draft_intention))
        .route("/pr/risk", post(handlers::pr_risk))
        .route("/fuel/diff/route", post(handlers::fuel_diff_route))
        .route(
            "/orchestrate/intention-confirmed",
            post(handlers::orchestrate_intention_confirmed),
        )
        .route(
            "/orchestrate/github-event",
            post(handlers::orchestrate_github_event),
        )
        .route(
            "/orchestrate/rollback",
            post(handlers::orchestrate_rollback),
        )
        .route_layer(from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ))
        .route_layer(from_fn_with_state(
            state.clone(),
            middleware::rate_limit_middleware,
        ))
        .route_layer(from_fn_with_state(
            state.clone(),
            middleware::idempotency_middleware,
        ));

    let app = Router::new()
        .route("/health", get(handlers::health))
        .nest("/v1", protected_v1)
        .layer(from_fn(middleware::request_id_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);
    app
}

#[cfg(test)]
mod tests {
    use super::{build_app, AppState};
    use crate::{
        config::{Config, IdempotencyBackend},
        policy::PolicySet,
        state_store::StateStore,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use std::{env, path::PathBuf, sync::Arc};
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_config() -> Config {
        let policy_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../policy/policy-set.v1.1.json");
        Config {
            host: "127.0.0.1".into(),
            port: 18080,
            policy_set_path: policy_path.display().to_string(),
            supabase_url: None,
            supabase_service_role_key: None,
            supabase_jwt_secret: Some("jwt-secret".into()),
            default_tenant_id: Some("tenant-test".into()),
            default_app_id: Some("edge-control-test".into()),
            default_user_id: Some("user-test".into()),
            obs_api_base_url: None,
            obs_api_token: None,
            code247_base_url: "http://127.0.0.1:4001".into(),
            code247_intentions_token: None,
            supabase_jwks_url: None,
            supabase_jwt_audience: None,
            internal_api_token: Some("internal-test-token".into()),
            rate_limit_window_seconds: 60,
            rate_limit_max_requests: 120,
            idempotency_ttl_seconds: 900,
            idempotency_backend: IdempotencyBackend::Sqlite,
            state_db_path: env::temp_dir()
                .join(format!("edge-control-test-{}.db", Uuid::new_v4()))
                .display()
                .to_string(),
            resilience_max_retries: 1,
            resilience_initial_backoff_ms: 10,
            resilience_circuit_failures: 2,
            resilience_circuit_open_seconds: 5,
        }
    }

    fn test_state() -> Arc<AppState> {
        let config = test_config();
        let policy_set = PolicySet::load(&config.policy_set_path).expect("policy");
        let state_store = StateStore::from_config(&config).expect("state db");
        Arc::new(AppState::new(config, policy_set, state_store))
    }

    #[tokio::test]
    async fn health_contract_is_stable() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(payload["status"], "ok");
        assert_eq!(
            payload["output_schema"],
            "https://logline.world/schemas/response-envelope.v1.schema.json"
        );
    }

    #[tokio::test]
    async fn protected_route_requires_bearer_token() {
        let app = build_app(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/intention/draft")
                    .header("content-type", "application/json")
                    .header("x-idempotency-key", "idem-1")
                    .body(Body::from(
                        r#"{"version":"intention.draft.request.v1","intent_text":"fix auth bug","context":{"repo":"code247","default_branch":"main"}}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn draft_intention_returns_contract_and_rejects_duplicate_idempotency() {
        let app = build_app(test_state());
        let request_body = r#"{"version":"intention.draft.request.v1","intent_text":"fix auth bug","context":{"repo":"code247","default_branch":"main"}}"#;

        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/intention/draft")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer internal-test-token")
                    .header("x-idempotency-key", "idem-2")
                    .body(Body::from(request_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = to_bytes(first.into_body(), usize::MAX).await.expect("body");
        let first_payload: serde_json::Value = serde_json::from_slice(&first_body).expect("json");
        assert_eq!(first_payload["version"], "draft-intention.v1");

        let second = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/intention/draft")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer internal-test-token")
                    .header("x-idempotency-key", "idem-2")
                    .body(Body::from(request_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(second.status(), StatusCode::CONFLICT);
    }
}
