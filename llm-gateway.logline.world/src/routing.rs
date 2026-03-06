//! Routing logic for llm-gateway
//! Route candidate generation and mode/task classification.

use crate::config::{provider_ready, Config, LlmRoute, ProviderConfig, ReliabilityPolicy};
use crate::types::{
    ChatMessage, ExecutionBudget, ModelProfile, RouteCandidate, RouteMode, TaskClass,
};
use std::time::Duration;

/// Creates genius-tier candidates (best reasoning models, expensive)
pub fn genius_candidates(config: &Config) -> Vec<RouteCandidate> {
    let matrix = &config.model_matrix;
    let mut out = Vec::new();

    // Anthropic Opus first for genius-tier (best reasoning)
    if let Some(cfg) = Some(&config.premium.anthropic).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "anthropic".into(),
            model: matrix.anthropic.genius.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            cost_tier: "high".into(),
            decision_hint: "candidate=genius:anthropic".into(),
        });
    }
    // OpenAI as fallback
    if let Some(cfg) = Some(&config.premium.openai).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "openai".into(),
            model: matrix.openai.genius.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".into()),
            cost_tier: "high".into(),
            decision_hint: "candidate=genius:openai".into(),
        });
    }
    // Gemini Pro as last fallback
    if let Some(cfg) = Some(&config.premium.gemini).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "gemini".into(),
            model: matrix.gemini.genius.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com".into()),
            cost_tier: "medium".into(),
            decision_hint: "candidate=genius:gemini".into(),
        });
    }
    out
}

/// Creates fast-tier candidates (cheapest premium models)
pub fn fast_candidates(config: &Config) -> Vec<RouteCandidate> {
    let matrix = &config.model_matrix;
    let mut out = Vec::new();

    // Gemini Flash first (cheapest)
    if let Some(cfg) = Some(&config.premium.gemini).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "gemini".into(),
            model: matrix.gemini.fast.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com".into()),
            cost_tier: "low".into(),
            decision_hint: "candidate=fast:gemini".into(),
        });
    }
    // Anthropic Haiku (5-10x cheaper than Sonnet)
    if let Some(cfg) = Some(&config.premium.anthropic).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "anthropic".into(),
            model: matrix.anthropic.fast.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            cost_tier: "low".into(),
            decision_hint: "candidate=fast:anthropic".into(),
        });
    }
    // OpenAI chat model
    if let Some(cfg) = Some(&config.premium.openai).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "openai".into(),
            model: matrix.openai.fast.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".into()),
            cost_tier: "low".into(),
            decision_hint: "candidate=fast:openai".into(),
        });
    }
    out
}

/// Creates code-tier candidates (local Qwen coder first, premium code models as fallback)
pub fn code_candidates(
    config: &Config,
    routes: &[LlmRoute],
    requested_model: Option<&str>,
) -> Vec<RouteCandidate> {
    let matrix = &config.model_matrix;
    let mut out = Vec::new();

    let requested = requested_model
        .map(|m| m.trim().to_lowercase())
        .filter(|m| !m.is_empty() && m != "auto" && m != "default");
    let mut ranked_routes: Vec<&LlmRoute> = routes.iter().collect();
    ranked_routes.sort_by_key(|route| local_route_rank(route, requested.as_deref()));

    for route in ranked_routes {
        out.push(RouteCandidate {
            provider: "local".into(),
            model: route.model.clone(),
            upstream_url: route.url.clone(),
            cost_tier: "free".into(),
            decision_hint: format!("candidate=code:local:{}", route.name),
        });
    }

    // Premium code models as fallback
    if let Some(cfg) = Some(&config.premium.anthropic).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "anthropic".into(),
            model: matrix.anthropic.code.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            cost_tier: "medium".into(),
            decision_hint: "candidate=code:anthropic".into(),
        });
    }
    if let Some(cfg) = Some(&config.premium.openai).filter(|c| provider_ready(c)) {
        out.push(RouteCandidate {
            provider: "openai".into(),
            model: matrix.openai.code.clone(),
            upstream_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com".into()),
            cost_tier: "medium".into(),
            decision_hint: "candidate=code:openai".into(),
        });
    }
    out
}

fn local_route_rank(route: &LlmRoute, requested_model: Option<&str>) -> u8 {
    let name_l = route.name.to_lowercase();
    let model_l = route.model.to_lowercase();
    let alias_l: Vec<String> = route.aliases.iter().map(|a| a.to_lowercase()).collect();

    if let Some(req) = requested_model {
        let req_l = req.to_lowercase();
        if model_l == req_l || name_l == req_l || alias_l.iter().any(|a| a == &req_l) {
            return 0;
        }
        if model_l.contains(&req_l)
            || name_l.contains(&req_l)
            || alias_l.iter().any(|a| a.contains(&req_l))
        {
            return 1;
        }
    }

    let is_qwen = model_l.contains("qwen");
    let is_coder = model_l.contains("coder") || alias_l.iter().any(|a| a.contains("coder"));
    if is_qwen && is_coder {
        return 2;
    }
    if is_qwen {
        return 3;
    }
    4
}

pub fn execution_budget(policy: &ReliabilityPolicy, mode: &RouteMode) -> ExecutionBudget {
    let max_attempts = match mode {
        RouteMode::Genius | RouteMode::Fast => policy.max_attempts_premium,
        RouteMode::Code => policy.max_attempts_auto,
    };
    ExecutionBudget {
        max_attempts,
        max_local_attempts: policy.max_local_attempts_auto,
        max_total: Duration::from_millis(policy.max_total_timeout_ms),
    }
}

pub fn parse_mode(mode: Option<&str>, default_mode: &str) -> RouteMode {
    match mode.unwrap_or(default_mode).to_lowercase().as_str() {
        "genius" | "premium" => RouteMode::Genius,
        "fast" => RouteMode::Fast,
        "code" | "auto" | "local" => RouteMode::Code,
        _ => RouteMode::Code,
    }
}

pub fn route_mode_name(mode: &RouteMode) -> &'static str {
    match mode {
        RouteMode::Genius => "genius",
        RouteMode::Fast => "fast",
        RouteMode::Code => "code",
    }
}

pub fn classify_task(task_hint: Option<&str>, messages: &[ChatMessage]) -> TaskClass {
    let mut text = task_hint.unwrap_or_default().to_lowercase();
    for m in messages {
        text.push(' ');
        text.push_str(&m.content.to_lowercase());
    }

    if text.contains("classifica")
        || text.contains("classify")
        || text.contains("organizar")
        || text.contains("metadata")
        || text.contains("background")
        || text.contains("lento")
    {
        return TaskClass::BackgroundClassification;
    }
    if text.contains("code")
        || text.contains("rust")
        || text.contains("debug")
        || text.contains("refactor")
    {
        return TaskClass::Coding;
    }
    if text.contains("plan") || text.contains("arquitetura") || text.contains("roadmap") {
        return TaskClass::Planning;
    }
    if text.contains("security") || text.contains("compliance") || text.contains("critic") {
        return TaskClass::Critical;
    }
    TaskClass::General
}

pub fn task_name(task: &TaskClass) -> &'static str {
    match task {
        TaskClass::BackgroundClassification => "background_classification",
        TaskClass::Coding => "coding",
        TaskClass::Planning => "planning",
        TaskClass::Critical => "critical",
        TaskClass::General => "general",
    }
}

pub fn candidate_key(candidate: &RouteCandidate) -> String {
    format!(
        "{}|{}|{}",
        candidate.provider, candidate.upstream_url, candidate.model
    )
}

pub fn is_retryable_error(message: &str) -> bool {
    let m = message.to_lowercase();
    if m.contains("api key not configured") || m.contains("provider_auth_error") {
        return false;
    }
    m.contains("429")
        || m.contains("502")
        || m.contains("503")
        || m.contains("504")
        || m.contains("timeout")
        || m.contains("timed out")
        || m.contains("connection")
        || m.contains("temporar")
        || m.contains("upstream error")
}

// Model scoring helpers

pub fn score_local_model(route_name: &str, model: &str) -> ModelProfile {
    let m = model.to_lowercase();
    let code = if m.contains("coder") || m.contains("code") {
        5
    } else {
        3
    };
    let genius = if m.contains("70b") || m.contains("32b") {
        4
    } else {
        3
    };
    let speed = if m.contains("1b") || m.contains("3b") {
        5
    } else if m.contains("7b") {
        4
    } else {
        3
    };

    ModelProfile {
        provider: format!("local:{}", route_name),
        model: model.to_string(),
        tier: "local".into(),
        cost: 1,
        genius,
        code,
        speed,
        tags: vec!["zero_cost".into(), "background_friendly".into()],
    }
}

pub fn premium_profiles(config: &Config) -> Vec<ModelProfile> {
    let mut v = Vec::new();

    if provider_ready(&config.premium.openai) {
        v.push(score_premium_model(
            "openai",
            configured_or_default_model("openai", &config.premium.openai),
        ));
    }

    if provider_ready(&config.premium.anthropic) {
        v.push(score_premium_model(
            "anthropic",
            configured_or_default_model("anthropic", &config.premium.anthropic),
        ));
    }

    if provider_ready(&config.premium.gemini) {
        v.push(score_premium_model(
            "gemini",
            configured_or_default_model("gemini", &config.premium.gemini),
        ));
    }

    v
}

pub fn configured_or_default_model(provider: &str, cfg: &ProviderConfig) -> String {
    cfg.default_model.clone().unwrap_or_else(|| match provider {
        "openai" => "gpt-5-mini".into(),
        "anthropic" => "claude-sonnet-4-20250514".into(),
        "gemini" => "gemini-2.5-flash".into(),
        _ => "unknown-model".into(),
    })
}

pub fn score_premium_model(provider: &str, model: String) -> ModelProfile {
    let m = model.to_lowercase();
    let (cost, genius, code, speed) = match provider {
        "openai" => {
            if m.contains("5.2") || m.contains("o1") {
                (5, 5, 4, 3)
            } else if m.contains("5.1") || m.contains("turbo") {
                (3, 4, 4, 4)
            } else {
                (2, 3, 3, 5)
            }
        }
        "anthropic" => {
            if m.contains("opus") {
                (5, 5, 5, 2)
            } else if m.contains("sonnet") {
                (3, 4, 5, 4)
            } else {
                (1, 3, 3, 5)
            }
        }
        "gemini" => {
            if m.contains("pro") || m.contains("3.1") {
                (3, 4, 4, 3)
            } else {
                (1, 3, 3, 5)
            }
        }
        _ => (2, 3, 3, 4),
    };

    ModelProfile {
        provider: provider.to_string(),
        model,
        tier: "premium".into(),
        cost,
        genius,
        code,
        speed,
        tags: vec![],
    }
}

pub fn dedupe_profiles(mut models: Vec<ModelProfile>) -> Vec<ModelProfile> {
    models.sort_by(|a, b| a.model.cmp(&b.model));
    models.dedup_by(|a, b| a.model == b.model && a.provider == b.provider);
    models
}
