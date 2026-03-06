mod auth;
mod config;
mod fuel;
mod handlers;
mod middleware;
mod models;
mod orchestration;
mod policy;

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

use crate::{config::Config, middleware::RateBucket, policy::PolicySet};

pub struct AppState {
    pub config: Config,
    pub policy_set: PolicySet,
    pub http_client: reqwest::Client,
    rate_buckets: Mutex<HashMap<String, RateBucket>>,
    idempotency_keys: Mutex<HashMap<String, Instant>>,
}

impl AppState {
    fn new(config: Config, policy_set: PolicySet) -> Self {
        Self {
            config,
            policy_set,
            http_client: reqwest::Client::new(),
            rate_buckets: Mutex::new(HashMap::new()),
            idempotency_keys: Mutex::new(HashMap::new()),
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

    pub async fn register_idempotency_key(&self, key: &str) -> bool {
        let ttl = Duration::from_secs(self.config.idempotency_ttl_seconds);
        let now = Instant::now();

        let mut keys = self.idempotency_keys.lock().await;
        keys.retain(|_, seen_at| now.duration_since(*seen_at) <= ttl);

        if keys.contains_key(key) {
            return false;
        }

        keys.insert(key.to_string(), now);
        true
    }

    pub async fn remove_idempotency_key(&self, key: &str) {
        let mut keys = self.idempotency_keys.lock().await;
        keys.remove(key);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;
    let bind_addr = config.bind_addr();
    let policy_path = policy::resolve_policy_set_path(&config.policy_set_path);
    let policy_set = PolicySet::load(&policy_path)?;
    info!(policy_path=%policy_path, policy_version=%policy_set.version, "loaded policy set");

    let state = Arc::new(AppState::new(config, policy_set));

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

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!(%bind_addr, "edge-control listening");

    axum::serve(listener, app).await?;
    Ok(())
}
