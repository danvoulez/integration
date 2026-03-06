//! Supabase Realtime broadcast for job status updates.
//!
//! Used by services to broadcast status changes to connected clients.

use crate::{Error, Result, SupabaseClient};
use serde::Serialize;

impl SupabaseClient {
    /// Broadcast a message to a Realtime channel.
    ///
    /// This uses HTTP broadcast, not WebSocket (fire-and-forget).
    /// For services that need to notify connected clients of status changes.
    ///
    /// # Channel Naming Convention
    ///
    /// - `code247:jobs:{tenant_id}` — Job status updates
    /// - `llm-gateway:health` — Provider health changes
    ///
    /// # Example (code247 service)
    ///
    /// ```ignore
    /// client.broadcast(
    ///     &format!("code247:jobs:{}", tenant_id),
    ///     "job_status",
    ///     &json!({
    ///         "job_id": "job-123",
    ///         "status": "IN_PROGRESS",
    ///         "stage": "coding",
    ///         "progress": 50
    ///     })
    /// ).await?;
    /// ```
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline broadcast --channel "code247:jobs:tenant-123" --event job_status --payload '{"job_id":"x"}'
    /// ```
    pub async fn broadcast<T: Serialize>(
        &self,
        channel: &str,
        event: &str,
        payload: &T,
    ) -> Result<()> {
        // Supabase Realtime HTTP broadcast endpoint
        let url = format!("{}/api/broadcast", self.realtime_url());

        let body = serde_json::json!({
            "channel": channel,
            "event": event,
            "payload": payload
        });

        let response = self
            .http()
            .post(&url)
            .headers(self.auth_headers())
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Realtime(format!("Broadcast failed: {}", text)));
        }

        tracing::debug!(channel, event, "Broadcast sent");
        Ok(())
    }

    /// Broadcast job status update (convenience wrapper).
    ///
    /// # CLI Usage
    ///
    /// ```bash
    /// logline jobs status --job job-123 --status IN_PROGRESS --stage coding
    /// ```
    pub async fn broadcast_job_status(
        &self,
        tenant_id: &str,
        job_id: &str,
        status: &str,
        stage: Option<&str>,
        progress: Option<u8>,
        error: Option<&str>,
    ) -> Result<()> {
        let channel = format!("code247:jobs:{}", tenant_id);
        let payload = JobStatusPayload {
            job_id: job_id.to_string(),
            status: status.to_string(),
            stage: stage.map(String::from),
            progress,
            error: error.map(String::from),
            timestamp: chrono::Utc::now(),
        };

        self.broadcast(&channel, "job_status", &payload).await
    }
}

/// Payload for job status broadcasts.
#[derive(Debug, Clone, Serialize)]
pub struct JobStatusPayload {
    pub job_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
