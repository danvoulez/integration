use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::{sync::Mutex, time::sleep};

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct CircuitBreakerState {
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

pub type CircuitBreakers = Mutex<HashMap<String, CircuitBreakerState>>;

pub async fn send_with_resilience<F>(
    config: &Config,
    breakers: &Arc<CircuitBreakers>,
    target: &str,
    build_request: F,
) -> Result<Response>
where
    F: Fn() -> RequestBuilder,
{
    ensure_circuit_closed(config, breakers, target).await?;

    let mut last_error: Option<anyhow::Error> = None;
    let max_attempts = config.resilience_max_retries.saturating_add(1);
    for attempt in 0..max_attempts {
        match build_request().send().await {
            Ok(response) if response.status().is_success() => {
                record_success(breakers, target).await;
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
                record_failure(config, breakers, target).await;
                return Err(anyhow!("downstream returned {status}: {body}"));
            }
            Err(err) if attempt + 1 < max_attempts => {
                last_error = Some(err.into());
            }
            Err(err) => {
                record_failure(config, breakers, target).await;
                return Err(err.into());
            }
        }

        let backoff_ms = config
            .resilience_initial_backoff_ms
            .saturating_mul(2_u64.saturating_pow(attempt));
        sleep(Duration::from_millis(backoff_ms.max(50))).await;
    }

    record_failure(config, breakers, target).await;
    Err(last_error.unwrap_or_else(|| anyhow!("request failed without concrete error")))
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

async fn ensure_circuit_closed(
    config: &Config,
    breakers: &Arc<CircuitBreakers>,
    target: &str,
) -> Result<()> {
    let now = Instant::now();
    let guard = breakers.lock().await;
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
    if config.resilience_circuit_open_seconds == 0 {
        return Ok(());
    }
    Ok(())
}

async fn record_success(breakers: &Arc<CircuitBreakers>, target: &str) {
    let mut guard = breakers.lock().await;
    guard.insert(target.to_string(), CircuitBreakerState::closed());
}

async fn record_failure(config: &Config, breakers: &Arc<CircuitBreakers>, target: &str) {
    let mut guard = breakers.lock().await;
    let state = guard
        .entry(target.to_string())
        .or_insert_with(CircuitBreakerState::closed);
    state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    if state.consecutive_failures >= config.resilience_circuit_failures.max(1) {
        state.open_until = Some(
            Instant::now() + Duration::from_secs(config.resilience_circuit_open_seconds.max(5)),
        );
        state.consecutive_failures = 0;
    }
}
