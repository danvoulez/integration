//! Batch processor worker for non-urgent LLM requests
//!
//! Processes queued batch jobs in the background, calling providers
//! and updating job status. Jobs get 50% cost reduction by being
//! processed asynchronously (within 24 hours).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tracing::{error, info, warn};

use crate::batch::{BatchJob, BatchQueue};
use crate::config::{Config, LocalRequestParams};
use crate::types::{ChatMessage, OllamaChatRequest, OllamaChatResponse, OllamaOptions};

/// Batch worker configuration
pub struct BatchWorkerConfig {
    /// How often to check for pending jobs (default: 30s)
    pub poll_interval: Duration,
    /// Maximum jobs to process per cycle (default: 10)
    pub batch_size: usize,
    /// Retry delay after failure before marking job failed (default: 60s)
    pub retry_delay: Duration,
    /// Maximum retries before permanent failure
    pub max_retries: u32,
}

impl Default for BatchWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(30),
            batch_size: 10,
            retry_delay: Duration::from_secs(60),
            max_retries: 3,
        }
    }
}

/// Start the batch processor worker as a background task
pub fn spawn_batch_worker(
    db_path: String,
    client: reqwest::Client,
    config: Arc<Config>,
) -> tokio::task::JoinHandle<()> {
    let worker_config = BatchWorkerConfig::default();

    tokio::spawn(async move {
        // Create our own BatchQueue instance from db_path
        let queue = match BatchQueue::new(&db_path) {
            Ok(q) => q,
            Err(e) => {
                error!("Failed to initialize batch queue: {}", e);
                return;
            }
        };

        info!(
            "Batch processor worker started (poll_interval={}s, batch_size={})",
            worker_config.poll_interval.as_secs(),
            worker_config.batch_size
        );

        loop {
            // Process pending batch jobs
            match process_batch_cycle(&queue, &client, &config, &worker_config).await {
                Ok(processed) => {
                    if processed > 0 {
                        info!("Batch worker: processed {} jobs", processed);
                    }
                }
                Err(e) => {
                    error!("Batch worker error: {}", e);
                }
            }

            // Wait before next cycle
            tokio::time::sleep(worker_config.poll_interval).await;
        }
    })
}

/// Run one cycle of batch processing
async fn process_batch_cycle(
    queue: &BatchQueue,
    client: &reqwest::Client,
    config: &Config,
    worker_config: &BatchWorkerConfig,
) -> anyhow::Result<usize> {
    let pending = queue.get_pending(worker_config.batch_size)?;

    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0;

    for job in pending {
        // Mark as processing
        if let Err(e) = queue.mark_processing(&job.id) {
            warn!("Failed to mark job {} as processing: {}", job.id, e);
            continue;
        }

        // Process the job
        match process_single_job(client, config, &job).await {
            Ok(response) => {
                // Mark completed
                if let Err(e) = queue.mark_completed(&job.id, &response) {
                    error!("Failed to mark job {} as completed: {}", job.id, e);
                } else {
                    info!(
                        "Batch job {} completed (provider={}, model={})",
                        job.id, job.provider, job.model
                    );
                    processed += 1;

                    // Call webhook if specified
                    if let Some(callback_url) = &job.callback_url {
                        if let Err(e) = send_webhook(client, callback_url, &job.id, &response).await
                        {
                            warn!("Webhook failed for job {}: {}", job.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Batch job {} failed: {}", job.id, e);
                if let Err(mark_err) = queue.mark_failed(&job.id, &e.to_string()) {
                    error!("Failed to mark job {} as failed: {}", job.id, mark_err);
                }
            }
        }
    }

    Ok(processed)
}

/// Process a single batch job by calling the appropriate provider
async fn process_single_job(
    client: &reqwest::Client,
    config: &Config,
    job: &BatchJob,
) -> anyhow::Result<String> {
    let provider = job.provider.to_lowercase();
    let timeout = Duration::from_millis(config.reliability.per_attempt_timeout_ms);

    match provider.as_str() {
        "openai" => {
            let api_key = config
                .premium
                .openai
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key not set"))?;
            let base_url = config
                .premium
                .openai
                .base_url
                .as_deref()
                .unwrap_or("https://api.openai.com");

            call_openai_batch(client, base_url, api_key, job, timeout).await
        }
        "anthropic" => {
            let api_key = config
                .premium
                .anthropic
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Anthropic API key not set"))?;
            let base_url = config
                .premium
                .anthropic
                .base_url
                .as_deref()
                .unwrap_or("https://api.anthropic.com");

            call_anthropic_batch(client, base_url, api_key, job, timeout).await
        }
        "gemini" => {
            let api_key = config
                .premium
                .gemini
                .api_key
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Gemini API key not set"))?;

            call_gemini_batch(client, api_key, job, timeout).await
        }
        "ollama" | "local" => {
            // Find local route for the model
            let route = config
                .routes
                .iter()
                .find(|r| r.model == job.model || r.aliases.contains(&job.model))
                .ok_or_else(|| anyhow::anyhow!("No local route for model {}", job.model))?;
            let params = config.local_params_for_route(&route.url, &route.model);

            call_ollama_batch(client, &route.url, &route.model, job, timeout, &params).await
        }
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider)),
    }
}

/// Call OpenAI for batch job
async fn call_openai_batch(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    job: &BatchJob,
    timeout: Duration,
) -> anyhow::Result<String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

    let body = json!({
        "model": job.model,
        "messages": job.messages,
        "temperature": job.temperature.unwrap_or(0.5),
        "max_tokens": job.max_tokens.unwrap_or(1024),
        "store": true,
        "metadata": {"source": "llm-gateway-batch"}
    });

    let response = client
        .post(&url)
        .timeout(timeout)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("OpenAI error ({}): {}", status, body));
    }

    let parsed: serde_json::Value = response.json().await?;
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

/// Call Anthropic for batch job
async fn call_anthropic_batch(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    job: &BatchJob,
    timeout: Duration,
) -> anyhow::Result<String> {
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    // Extract system message and convert messages to Anthropic format
    let (system, messages) = convert_messages_for_anthropic(&job.messages);

    let mut body = json!({
        "model": job.model,
        "messages": messages,
        "max_tokens": job.max_tokens.unwrap_or(1024),
    });

    if let Some(temp) = job.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(system_text) = system {
        // Use cache_control for system prompt if it's long enough
        if system_text.len() >= 1024 {
            body["system"] = json!([{
                "type": "text",
                "text": system_text,
                "cache_control": {"type": "ephemeral"}
            }]);
        } else {
            body["system"] = json!(system_text);
        }
    }

    let response = client
        .post(&url)
        .timeout(timeout)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Anthropic error ({}): {}", status, body));
    }

    let parsed: serde_json::Value = response.json().await?;
    let content = parsed["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

/// Convert messages for Anthropic format (extract system, convert roles)
fn convert_messages_for_anthropic(
    messages: &[ChatMessage],
) -> (Option<String>, Vec<serde_json::Value>) {
    let mut system = None;
    let mut converted = Vec::new();

    for msg in messages {
        if msg.role.eq_ignore_ascii_case("system") {
            system = Some(msg.content.clone());
        } else {
            converted.push(json!({
                "role": if msg.role.eq_ignore_ascii_case("assistant") { "assistant" } else { "user" },
                "content": msg.content
            }));
        }
    }

    (system, converted)
}

/// Call Gemini for batch job
async fn call_gemini_batch(
    client: &reqwest::Client,
    api_key: &str,
    job: &BatchJob,
    timeout: Duration,
) -> anyhow::Result<String> {
    let model = &job.model;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    // Extract system instruction for implicit caching (≥1024 tokens)
    let (system_instruction, contents) = convert_messages_for_gemini(&job.messages);

    let mut body = json!({
        "contents": contents,
        "generationConfig": {
            "temperature": job.temperature.unwrap_or(0.5),
            "maxOutputTokens": job.max_tokens.unwrap_or(1024),
        }
    });

    // Add system_instruction for implicit caching if present and long enough
    if let Some(sysins) = system_instruction {
        if sysins.len() >= 1024 {
            body["system_instruction"] = json!({"parts": [{"text": sysins}]});
        }
    }

    let response = client
        .post(&url)
        .timeout(timeout)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Gemini error ({}): {}", status, body));
    }

    let parsed: serde_json::Value = response.json().await?;
    let content = parsed["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

/// Convert messages for Gemini format (extract system, convert roles)
fn convert_messages_for_gemini(
    messages: &[ChatMessage],
) -> (Option<String>, Vec<serde_json::Value>) {
    let mut system_instruction = None;
    let mut contents = Vec::new();

    for msg in messages {
        if msg.role.eq_ignore_ascii_case("system") {
            system_instruction = Some(msg.content.clone());
        } else {
            let role = if msg.role.eq_ignore_ascii_case("assistant") {
                "model"
            } else {
                "user"
            };
            contents.push(json!({
                "role": role,
                "parts": [{"text": msg.content}]
            }));
        }
    }

    (system_instruction, contents)
}

/// Call local Ollama for batch job
async fn call_ollama_batch(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    job: &BatchJob,
    timeout: Duration,
    local_params: &LocalRequestParams,
) -> anyhow::Result<String> {
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let body = OllamaChatRequest {
        model: model.to_string(),
        messages: job.messages.clone(),
        stream: false,
        keep_alive: local_params.keep_alive.clone(),
        options: OllamaOptions {
            temperature: job.temperature.unwrap_or(0.5),
            num_predict: job.max_tokens.unwrap_or(1024),
            num_ctx: local_params.options.num_ctx,
            num_batch: local_params.options.num_batch,
            num_thread: local_params.options.num_thread,
            num_gpu: local_params.options.num_gpu,
            top_k: local_params.options.top_k,
            top_p: local_params.options.top_p,
            repeat_penalty: local_params.options.repeat_penalty,
        },
    };

    let response = client
        .post(&url)
        .timeout(timeout)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Ollama error ({}): {}", status, body));
    }

    let parsed: OllamaChatResponse = response.json().await?;
    let content = parsed
        .message
        .map(|m| m.content)
        .or(parsed.response)
        .unwrap_or_default();

    Ok(content)
}

/// Send webhook notification when job completes
async fn send_webhook(
    client: &reqwest::Client,
    callback_url: &str,
    job_id: &str,
    response: &str,
) -> anyhow::Result<()> {
    let payload = json!({
        "job_id": job_id,
        "status": "completed",
        "response": response,
        "completed_at": chrono::Utc::now().to_rfc3339(),
    });

    let resp = client
        .post(callback_url)
        .timeout(Duration::from_secs(10))
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow::anyhow!("Webhook returned {}", status));
    }

    info!("Webhook sent for job {} to {}", job_id, callback_url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages_anthropic() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "You are helpful".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "Hi!".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "How are you?".into(),
            },
        ];

        let (system, converted) = convert_messages_for_anthropic(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0]["role"], "user");
        assert_eq!(converted[1]["role"], "assistant");
        assert_eq!(converted[2]["role"], "user");
    }

    #[test]
    fn test_convert_messages_gemini() {
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: "Be concise".into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "Explain rust".into(),
            },
        ];

        let (system, contents) = convert_messages_for_gemini(&messages);

        assert_eq!(system, Some("Be concise".to_string()));
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
    }

    #[test]
    fn test_worker_config_default() {
        let config = BatchWorkerConfig::default();
        assert_eq!(config.poll_interval, Duration::from_secs(30));
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.max_retries, 3);
    }
}
