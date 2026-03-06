//! Common types for llm-gateway

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{atomic::AtomicU64, Mutex},
    time::Instant,
};
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub mode: Option<String>,
    pub task_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub request_id: String,
    pub output_schema: &'static str,
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
    #[serde(rename = "_lab")]
    pub lab_meta: LabMeta,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Serialize)]
pub struct LabMeta {
    pub route: String,
    pub upstream_url: String,
    pub model_used: String,
    pub mode_used: String,
    pub task_class: String,
    pub decision_path: Vec<String>,
    pub cost_tier: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct MatrixResponse {
    pub updated_at: String,
    pub models: Vec<ModelProfile>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelProfile {
    pub provider: String,
    pub model: String,
    pub tier: String,
    pub cost: u8,
    pub genius: u8,
    pub code: u8,
    pub speed: u8,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub request_id: String,
    pub output_schema: &'static str,
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
    pub options: OllamaOptions,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct OllamaOptions {
    pub temperature: f32,
    pub num_predict: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_batch: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_thread: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_gpu: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct OllamaChatResponse {
    pub message: Option<OllamaMessage>,
    pub response: Option<String>,
    pub total_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct OllamaMessage {
    pub content: String,
}

/// Routing mode determines how requests are handled.
///
/// New modes (genius/fast/code) provide explicit cost-quality tradeoffs.
#[derive(Clone, Debug, PartialEq)]
pub enum RouteMode {
    /// Best reasoning, complex analysis - uses opus/gpt-5.2 (expensive)
    Genius,
    /// Quick responses, simple queries - uses haiku/flash (cheap)
    Fast,
    /// Coding-optimized - local Qwen first, premium fallback (balanced)
    Code,
}

#[derive(Clone)]
pub enum TaskClass {
    BackgroundClassification,
    Coding,
    Planning,
    Critical,
    General,
}

pub struct RouteDecision {
    pub provider: String,
    pub model: String,
    pub upstream_url: String,
    pub mode_used: String,
    pub task_class: String,
    pub decision_path: Vec<String>,
    pub cost_tier: String,
}

#[derive(Clone)]
pub struct RouteCandidate {
    pub provider: String,
    pub model: String,
    pub upstream_url: String,
    pub cost_tier: String,
    pub decision_hint: String,
}

#[derive(Default)]
pub struct RouteHealth {
    pub consecutive_failures: u32,
    pub open_until: Option<Instant>,
}

pub struct ExecutionBudget {
    pub max_attempts: usize,
    pub max_local_attempts: usize,
    pub max_total: std::time::Duration,
}

pub struct GatewayMetrics {
    pub started: Instant,
    pub total_requests: AtomicU64,
    pub stream_requests: AtomicU64,
    pub non_stream_requests: AtomicU64,
    pub success_requests: AtomicU64,
    pub error_requests: AtomicU64,
    pub fallback_attempt_failures: AtomicU64,
    pub circuit_breaker_opens: AtomicU64,
    pub total_latency_ms: AtomicU64,
    pub max_latency_ms: AtomicU64,
    pub estimated_prompt_tokens_total: AtomicU64,
    pub estimated_completion_tokens_total: AtomicU64,
    pub local_requests_total: AtomicU64,
    pub local_queue_wait_ms_total: AtomicU64,
    pub local_queue_timeouts_total: AtomicU64,
    pub local_warmup_runs_total: AtomicU64,
    pub local_warmup_failures_total: AtomicU64,
    pub local_ollama_timing_samples_total: AtomicU64,
    pub local_ollama_load_ms_total: AtomicU64,
    pub local_ollama_prompt_eval_ms_total: AtomicU64,
    pub local_ollama_eval_ms_total: AtomicU64,
    pub fuel_supabase_emit_success_total: AtomicU64,
    pub fuel_supabase_emit_fail_total: AtomicU64,
    pub fuel_sqlite_fallback_writes_total: AtomicU64,
    pub fuel_settlement_runs_total: AtomicU64,
    pub fuel_settlement_failures_total: AtomicU64,
    pub fuel_settled_events_total: AtomicU64,
    pub fuel_local_energy_updates_total: AtomicU64,
    pub latency_samples_all: Mutex<VecDeque<u64>>,
    pub latency_samples_by_mode: Mutex<HashMap<String, VecDeque<u64>>>,
    pub latency_samples_by_provider: Mutex<HashMap<String, VecDeque<u64>>>,
    pub latency_samples_by_model: Mutex<HashMap<String, VecDeque<u64>>>,
    pub selected_by_provider: RwLock<HashMap<String, u64>>,
    pub selected_by_model: RwLock<HashMap<String, u64>>,
    pub error_by_provider: RwLock<HashMap<String, u64>>,
    pub error_by_model: RwLock<HashMap<String, u64>>,
    pub local_adaptive_profile_total: RwLock<HashMap<String, u64>>,
}

impl GatewayMetrics {
    const LATENCY_WINDOW_CAPACITY: usize = 1024;

    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            total_requests: AtomicU64::new(0),
            stream_requests: AtomicU64::new(0),
            non_stream_requests: AtomicU64::new(0),
            success_requests: AtomicU64::new(0),
            error_requests: AtomicU64::new(0),
            fallback_attempt_failures: AtomicU64::new(0),
            circuit_breaker_opens: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
            max_latency_ms: AtomicU64::new(0),
            estimated_prompt_tokens_total: AtomicU64::new(0),
            estimated_completion_tokens_total: AtomicU64::new(0),
            local_requests_total: AtomicU64::new(0),
            local_queue_wait_ms_total: AtomicU64::new(0),
            local_queue_timeouts_total: AtomicU64::new(0),
            local_warmup_runs_total: AtomicU64::new(0),
            local_warmup_failures_total: AtomicU64::new(0),
            local_ollama_timing_samples_total: AtomicU64::new(0),
            local_ollama_load_ms_total: AtomicU64::new(0),
            local_ollama_prompt_eval_ms_total: AtomicU64::new(0),
            local_ollama_eval_ms_total: AtomicU64::new(0),
            fuel_supabase_emit_success_total: AtomicU64::new(0),
            fuel_supabase_emit_fail_total: AtomicU64::new(0),
            fuel_sqlite_fallback_writes_total: AtomicU64::new(0),
            fuel_settlement_runs_total: AtomicU64::new(0),
            fuel_settlement_failures_total: AtomicU64::new(0),
            fuel_settled_events_total: AtomicU64::new(0),
            fuel_local_energy_updates_total: AtomicU64::new(0),
            latency_samples_all: Mutex::new(VecDeque::with_capacity(Self::LATENCY_WINDOW_CAPACITY)),
            latency_samples_by_mode: Mutex::new(HashMap::new()),
            latency_samples_by_provider: Mutex::new(HashMap::new()),
            latency_samples_by_model: Mutex::new(HashMap::new()),
            selected_by_provider: RwLock::new(HashMap::new()),
            selected_by_model: RwLock::new(HashMap::new()),
            error_by_provider: RwLock::new(HashMap::new()),
            error_by_model: RwLock::new(HashMap::new()),
            local_adaptive_profile_total: RwLock::new(HashMap::new()),
        }
    }

    pub fn observe_latency(&self, mode: &str, provider: &str, model: &str, latency_ms: u64) {
        if let Ok(mut all) = self.latency_samples_all.lock() {
            push_latency_sample(&mut all, latency_ms, Self::LATENCY_WINDOW_CAPACITY);
        }
        if let Ok(mut by_mode) = self.latency_samples_by_mode.lock() {
            let key = normalize_metric_label(mode);
            let window = by_mode
                .entry(key)
                .or_insert_with(|| VecDeque::with_capacity(Self::LATENCY_WINDOW_CAPACITY));
            push_latency_sample(window, latency_ms, Self::LATENCY_WINDOW_CAPACITY);
        }
        if let Ok(mut by_provider) = self.latency_samples_by_provider.lock() {
            let key = normalize_metric_label(provider);
            let window = by_provider
                .entry(key)
                .or_insert_with(|| VecDeque::with_capacity(Self::LATENCY_WINDOW_CAPACITY));
            push_latency_sample(window, latency_ms, Self::LATENCY_WINDOW_CAPACITY);
        }
        if let Ok(mut by_model) = self.latency_samples_by_model.lock() {
            let key = normalize_metric_label(model);
            let window = by_model
                .entry(key)
                .or_insert_with(|| VecDeque::with_capacity(Self::LATENCY_WINDOW_CAPACITY));
            push_latency_sample(window, latency_ms, Self::LATENCY_WINDOW_CAPACITY);
        }
    }

    pub fn latency_quantiles_snapshot(&self) -> LatencyQuantilesSnapshot {
        let all = self
            .latency_samples_all
            .lock()
            .ok()
            .map(|window| compute_latency_quantiles(&window))
            .unwrap_or_default();
        let by_mode = self
            .latency_samples_by_mode
            .lock()
            .ok()
            .map(|window_map| {
                window_map
                    .iter()
                    .map(|(mode, window)| (mode.clone(), compute_latency_quantiles(window)))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let by_provider = self
            .latency_samples_by_provider
            .lock()
            .ok()
            .map(|window_map| {
                window_map
                    .iter()
                    .map(|(provider, window)| (provider.clone(), compute_latency_quantiles(window)))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let by_model = self
            .latency_samples_by_model
            .lock()
            .ok()
            .map(|window_map| {
                window_map
                    .iter()
                    .map(|(model, window)| (model.clone(), compute_latency_quantiles(window)))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        LatencyQuantilesSnapshot {
            all,
            by_mode,
            by_provider,
            by_model,
        }
    }

    pub fn observe_local_queue_wait(&self, wait_ms: u64) {
        self.local_queue_wait_ms_total
            .fetch_add(wait_ms, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn observe_local_queue_timeout(&self) {
        self.local_queue_timeouts_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn observe_local_warmup(&self, success: bool) {
        self.local_warmup_runs_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !success {
            self.local_warmup_failures_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn observe_local_ollama_durations(
        &self,
        load_duration_ns: Option<u64>,
        prompt_eval_duration_ns: Option<u64>,
        eval_duration_ns: Option<u64>,
    ) {
        let mut sampled = false;
        if let Some(v) = load_duration_ns {
            self.local_ollama_load_ms_total
                .fetch_add(v / 1_000_000, std::sync::atomic::Ordering::Relaxed);
            sampled = true;
        }
        if let Some(v) = prompt_eval_duration_ns {
            self.local_ollama_prompt_eval_ms_total
                .fetch_add(v / 1_000_000, std::sync::atomic::Ordering::Relaxed);
            sampled = true;
        }
        if let Some(v) = eval_duration_ns {
            self.local_ollama_eval_ms_total
                .fetch_add(v / 1_000_000, std::sync::atomic::Ordering::Relaxed);
            sampled = true;
        }
        if sampled {
            self.local_ollama_timing_samples_total
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub async fn observe_local_adaptive_profile(&self, profile: &str) {
        let mut guard = self.local_adaptive_profile_total.write().await;
        let entry = guard.entry(normalize_metric_label(profile)).or_insert(0);
        *entry += 1;
    }

    pub async fn local_adaptive_profile_snapshot(&self) -> HashMap<String, u64> {
        self.local_adaptive_profile_total.read().await.clone()
    }
}

#[derive(Debug, Clone, Default)]
pub struct LatencyQuantiles {
    pub samples: usize,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct LatencyQuantilesSnapshot {
    pub all: LatencyQuantiles,
    pub by_mode: HashMap<String, LatencyQuantiles>,
    pub by_provider: HashMap<String, LatencyQuantiles>,
    pub by_model: HashMap<String, LatencyQuantiles>,
}

fn normalize_metric_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn push_latency_sample(window: &mut VecDeque<u64>, value: u64, max_samples: usize) {
    if max_samples == 0 {
        return;
    }
    while window.len() >= max_samples {
        let _ = window.pop_front();
    }
    window.push_back(value);
}

fn compute_latency_quantiles(window: &VecDeque<u64>) -> LatencyQuantiles {
    if window.is_empty() {
        return LatencyQuantiles::default();
    }
    let mut values = window.iter().copied().collect::<Vec<_>>();
    values.sort_unstable();
    LatencyQuantiles {
        samples: values.len(),
        p50_ms: percentile_nearest_rank(&values, 50),
        p95_ms: percentile_nearest_rank(&values, 95),
        p99_ms: percentile_nearest_rank(&values, 99),
    }
}

fn percentile_nearest_rank(sorted_values: &[u64], percentile: u64) -> u64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let n = sorted_values.len() as u64;
    let rank = ((percentile.saturating_mul(n)).saturating_add(99) / 100).max(1);
    let idx = (rank - 1) as usize;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::{compute_latency_quantiles, percentile_nearest_rank};
    use std::collections::VecDeque;

    #[test]
    fn percentile_uses_nearest_rank() {
        let sorted = vec![10_u64, 20, 30, 40, 50];
        assert_eq!(percentile_nearest_rank(&sorted, 50), 30);
        assert_eq!(percentile_nearest_rank(&sorted, 95), 50);
        assert_eq!(percentile_nearest_rank(&sorted, 99), 50);
    }

    #[test]
    fn quantiles_empty_window_defaults_to_zero() {
        let q = compute_latency_quantiles(&VecDeque::new());
        assert_eq!(q.samples, 0);
        assert_eq!(q.p50_ms, 0);
        assert_eq!(q.p95_ms, 0);
        assert_eq!(q.p99_ms, 0);
    }
}

#[derive(Clone, Copy, Default)]
pub struct FuelDelta {
    pub calls_total: u64,
    pub calls_success: u64,
    pub calls_failed: u64,
    pub calls_stream: u64,
    pub calls_non_stream: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}
