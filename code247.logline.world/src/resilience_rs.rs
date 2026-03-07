use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::{sync::Mutex, time::sleep};

#[derive(Clone, Copy, Debug)]
pub struct ResiliencePolicy {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub circuit_failures: u32,
    pub circuit_open_seconds: u64,
}

impl Default for ResiliencePolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_backoff_ms: 150,
            circuit_failures: 3,
            circuit_open_seconds: 15,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HttpResilience {
    policy: ResiliencePolicy,
    breakers: Arc<Mutex<HashMap<String, CircuitBreakerState>>>,
}

#[derive(Clone, Debug)]
struct CircuitBreakerState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

impl CircuitBreakerState {
    fn closed() -> Self {
        Self {
            consecutive_failures: 0,
            open_until: None,
        }
    }
}

impl HttpResilience {
    pub fn new(policy: ResiliencePolicy) -> Self {
        Self {
            policy,
            breakers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn send<F>(&self, target: &str, build_request: F) -> Result<Response>
    where
        F: Fn() -> RequestBuilder,
    {
        self.ensure_circuit_closed(target).await?;

        let mut last_error: Option<anyhow::Error> = None;
        let max_attempts = self.policy.max_retries.saturating_add(1);
        for attempt in 0..max_attempts {
            match build_request().send().await {
                Ok(response) if response.status().is_success() => {
                    self.record_success(target).await;
                    return Ok(response);
                }
                Ok(response)
                    if is_retryable_status(response.status()) && attempt + 1 < max_attempts =>
                {
                    last_error = Some(anyhow!("retryable status {}", response.status()));
                }
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    self.record_failure(target).await;
                    return Err(anyhow!("downstream returned {status}: {body}"));
                }
                Err(err) if attempt + 1 < max_attempts => {
                    last_error = Some(err.into());
                }
                Err(err) => {
                    self.record_failure(target).await;
                    return Err(err.into());
                }
            }

            let backoff_ms = self
                .policy
                .initial_backoff_ms
                .saturating_mul(2_u64.saturating_pow(attempt))
                .max(50);
            sleep(Duration::from_millis(backoff_ms)).await;
        }

        self.record_failure(target).await;
        Err(last_error.unwrap_or_else(|| anyhow!("request failed without concrete error")))
    }

    async fn ensure_circuit_closed(&self, target: &str) -> Result<()> {
        let now = Instant::now();
        let guard = self.breakers.lock().await;
        if let Some(state) = guard.get(target) {
            if let Some(open_until) = state.open_until {
                if open_until > now {
                    return Err(anyhow!(
                        "circuit open for target '{}' for another {}ms",
                        target,
                        open_until.duration_since(now).as_millis()
                    ));
                }
            }
        }
        Ok(())
    }

    async fn record_success(&self, target: &str) {
        let mut guard = self.breakers.lock().await;
        guard.insert(target.to_string(), CircuitBreakerState::closed());
    }

    async fn record_failure(&self, target: &str) {
        let mut guard = self.breakers.lock().await;
        let state = guard
            .entry(target.to_string())
            .or_insert_with(CircuitBreakerState::closed);
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if state.consecutive_failures >= self.policy.circuit_failures.max(1) {
            state.open_until = Some(
                Instant::now()
                    + Duration::from_secs(self.policy.circuit_open_seconds.max(5)),
            );
            state.consecutive_failures = 0;
        }
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

#[cfg(test)]
mod tests {
    use super::{HttpResilience, ResiliencePolicy};
    use anyhow::Result;
    use axum::{routing::get, Router};
    use reqwest::StatusCode;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn retries_transient_http_failures() -> Result<()> {
        let hits = Arc::new(AtomicUsize::new(0));
        let state = hits.clone();
        let app = Router::new().route(
            "/",
            get(move || {
                let state = state.clone();
                async move {
                    let current = state.fetch_add(1, Ordering::SeqCst);
                    if current == 0 {
                        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "retry")
                    } else {
                        (axum::http::StatusCode::OK, "ok")
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server");
        });

        let client = reqwest::Client::new();
        let resilience = HttpResilience::new(ResiliencePolicy {
            max_retries: 2,
            initial_backoff_ms: 10,
            circuit_failures: 3,
            circuit_open_seconds: 5,
        });

        let response = resilience
            .send("test.retry", || client.get(format!("http://{addr}/")))
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(hits.load(Ordering::SeqCst) >= 2);
        Ok(())
    }
}
