use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::{header::HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::{auth, models::ErrorResponseV1, AppState};

pub const REQUEST_ID_HEADER: &str = "x-request-id";
pub const IDEMPOTENCY_HEADER: &str = "x-idempotency-key";

#[derive(Clone, Debug)]
pub struct RequestContext {
    pub request_id: String,
}

pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    request.extensions_mut().insert(RequestContext {
        request_id: request_id.clone(),
    });

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }

    response
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    match auth::validate_headers(request.headers(), &state.config) {
        Ok(ctx) => {
            request.extensions_mut().insert(ctx);
            next.run(request).await
        }
        Err(err) => {
            let request_id = request
                .extensions()
                .get::<RequestContext>()
                .map(|ctx| ctx.request_id.clone());
            (
                StatusCode::UNAUTHORIZED,
                Json(err.with_request_id(request_id)),
            )
                .into_response()
        }
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let client_key = request
        .headers()
        .get("x-calling-app")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .or_else(|| {
            request
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "anonymous".into());

    if !state.consume_rate_slot(&client_key).await {
        let request_id = request
            .extensions()
            .get::<RequestContext>()
            .map(|ctx| ctx.request_id.clone());
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponseV1::new(
                request_id,
                "rate_limited",
                "Rate limit exceeded",
            )),
        )
            .into_response();
    }

    next.run(request).await
}

pub async fn idempotency_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    if method != Method::POST || !path.starts_with("/v1/") {
        return next.run(request).await;
    }

    let Some(key) = request
        .headers()
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .filter(|v| !v.trim().is_empty())
    else {
        let request_id = request
            .extensions()
            .get::<RequestContext>()
            .map(|ctx| ctx.request_id.clone());
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponseV1::new(
                request_id,
                "invalid_request",
                "Missing x-idempotency-key header",
            )),
        )
            .into_response();
    };

    if !state.register_idempotency_key(&key).await {
        let request_id = request
            .extensions()
            .get::<RequestContext>()
            .map(|ctx| ctx.request_id.clone());
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponseV1::new(
                request_id,
                "duplicate_request",
                "Duplicate idempotency key",
            )),
        )
            .into_response();
    }

    let mut response = next.run(request).await;

    if response.status().is_server_error() {
        state.remove_idempotency_key(&key).await;
    }

    if let Ok(value) = HeaderValue::from_str(&key) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(IDEMPOTENCY_HEADER), value);
    }

    response
}

pub struct RateBucket {
    pub hits: VecDeque<Instant>,
}

impl RateBucket {
    pub fn new() -> Self {
        Self {
            hits: VecDeque::new(),
        }
    }

    pub fn consume(&mut self, now: Instant, window: Duration, max_requests: u32) -> bool {
        while let Some(ts) = self.hits.front() {
            if now.duration_since(*ts) > window {
                let _ = self.hits.pop_front();
            } else {
                break;
            }
        }

        if self.hits.len() as u32 >= max_requests {
            return false;
        }

        self.hits.push_back(now);
        true
    }
}
