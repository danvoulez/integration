//! Provider implementations for llm-gateway
//! Handles calls to local Ollama, OpenAI, Anthropic, and Gemini.

use axum::{http::StatusCode, response::Json};
use serde_json::json;
use tracing::warn;

use crate::config::LocalRequestParams;
use crate::types::{
    ChatMessage, ErrorResponse, OllamaChatRequest, OllamaChatResponse, OllamaOptions,
};
use crate::utils::error_response;
use crate::AppState;

pub async fn call_local_ollama(
    state: &AppState,
    base_url: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    local_params: &LocalRequestParams,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let req = OllamaChatRequest {
        model: model.into(),
        messages,
        stream: false,
        keep_alive: local_params.keep_alive.clone(),
        options: OllamaOptions {
            temperature,
            num_predict: max_tokens,
            num_ctx: local_params.options.num_ctx,
            num_batch: local_params.options.num_batch,
            num_thread: local_params.options.num_thread,
            num_gpu: local_params.options.num_gpu,
            top_k: local_params.options.top_k,
            top_p: local_params.options.top_p,
            repeat_penalty: local_params.options.repeat_penalty,
        },
    };

    let response = state
        .client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            warn!(error = %e, "local ollama call failed");
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("local upstream error: {}", e),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("local upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let parsed: OllamaChatResponse = response.json().await.map_err(|e| {
        error_response(
            StatusCode::BAD_GATEWAY,
            &format!("local parse error: {}", e),
            "parse_error",
        )
    })?;
    state.metrics.observe_local_ollama_durations(
        parsed.load_duration,
        parsed.prompt_eval_duration,
        parsed.eval_duration,
    );

    Ok(parsed
        .message
        .map(|m| m.content)
        .or(parsed.response)
        .unwrap_or_default())
}

pub async fn call_openai(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    // OpenAI automatic prompt caching is FREE for prompts ≥1024 tokens
    // Adding store:true enables extended 24h retention
    let body = json!({
        "model": model,
        "messages": messages,
        "temperature": temperature.unwrap_or(0.5),
        "max_tokens": max_tokens.unwrap_or(1024),
        "store": true,  // Enable prompt caching with extended retention
        "metadata": {"source": "llm-gateway"}  // Routing hint for cache
    });

    let response = state
        .client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("openai upstream error: {}", e),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("openai upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let v: serde_json::Value = response.json().await.map_err(|e| {
        error_response(
            StatusCode::BAD_GATEWAY,
            &format!("openai parse error: {}", e),
            "parse_error",
        )
    })?;

    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    Ok(content)
}

pub async fn call_anthropic(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let mut system_blocks = Vec::new();
    let mut user_messages = Vec::new();
    for m in messages {
        if m.role == "system" {
            // Use array format with cache_control for prompt caching (90% cost reduction)
            system_blocks.push(json!({
                "type": "text",
                "text": m.content,
                "cache_control": {"type": "ephemeral"}
            }));
        } else {
            user_messages.push(json!({"role": if m.role == "assistant" {"assistant"} else {"user"}, "content": m.content}));
        }
    }

    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens.unwrap_or(1024),
        "messages": user_messages,
        "temperature": temperature.unwrap_or(0.5)
    });
    if !system_blocks.is_empty() {
        // Array format enables prompt caching
        body["system"] = json!(system_blocks);
    }

    let response = state
        .client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31") // Enable prompt caching
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("anthropic upstream error: {}", e),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("anthropic upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let v: serde_json::Value = response.json().await.map_err(|e| {
        error_response(
            StatusCode::BAD_GATEWAY,
            &format!("anthropic parse error: {}", e),
            "parse_error",
        )
    })?;

    let text = v["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|it| it["text"].as_str())
        .unwrap_or_default()
        .to_string();
    Ok(text)
}

pub async fn call_gemini(
    state: &AppState,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let url = format!(
        "{}/v1beta/models/{}:generateContent?key={}",
        base_url.trim_end_matches('/'),
        model,
        api_key
    );

    // Extract system messages for implicit caching (Gemini caches system_instruction automatically)
    let system_parts: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| json!({"text": m.content}))
        .collect();

    let contents: Vec<serde_json::Value> = messages
        .into_iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            json!({
                "role": if m.role == "assistant" { "model" } else { "user" },
                "parts": [{"text": m.content}]
            })
        })
        .collect();

    // Build request with system_instruction for implicit caching (≥1024 tokens cached automatically)
    let mut body = json!({
        "contents": contents,
        "generationConfig": {
            "temperature": temperature.unwrap_or(0.5),
            "maxOutputTokens": max_tokens.unwrap_or(1024)
        }
    });

    // Add system_instruction if present (enables implicit caching)
    if !system_parts.is_empty() {
        body["system_instruction"] = json!({"parts": system_parts});
    }

    let response = state
        .client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error_response(
                StatusCode::BAD_GATEWAY,
                &format!("gemini upstream error: {}", e),
                "upstream_error",
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            &format!("gemini upstream failure (status {status}): {body}"),
            "upstream_error",
        ));
    }

    let v: serde_json::Value = response.json().await.map_err(|e| {
        error_response(
            StatusCode::BAD_GATEWAY,
            &format!("gemini parse error: {}", e),
            "parse_error",
        )
    })?;

    let text = v["candidates"][0]["content"]["parts"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|part| part["text"].as_str())
        .unwrap_or_default()
        .to_string();

    Ok(text)
}
