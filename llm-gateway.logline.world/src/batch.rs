//! Batch API for non-urgent LLM requests (50% cost reduction)
//!
//! Supports batching requests to OpenAI and Anthropic for background processing.
//! Requests are queued locally and processed within 24 hours for 50% discount.

use crate::types::ChatMessage;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Batch job status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

/// A single batch job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJob {
    pub id: String,
    pub custom_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub callback_url: Option<String>,
    pub status: BatchStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub response: Option<String>,
    pub error: Option<String>,
}

/// Batch queue with SQLite persistence
pub struct BatchQueue {
    db_path: String,
}

impl BatchQueue {
    pub fn new(db_path: &str) -> anyhow::Result<Self> {
        let queue = Self {
            db_path: db_path.to_string(),
        };
        queue.init_db()?;
        Ok(queue)
    }

    fn init_db(&self) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS batch_jobs (
                id TEXT PRIMARY KEY,
                custom_id TEXT,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                messages TEXT NOT NULL,
                temperature REAL,
                max_tokens INTEGER,
                callback_url TEXT,
                status TEXT NOT NULL DEFAULT 'queued',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                response TEXT,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_batch_status ON batch_jobs(status);
            CREATE INDEX IF NOT EXISTS idx_batch_created ON batch_jobs(created_at);",
        )?;
        Ok(())
    }

    /// Queue a new batch job
    pub fn enqueue(&self, job: &BatchJob) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let messages_json = serde_json::to_string(&job.messages)?;

        conn.execute(
            "INSERT INTO batch_jobs (id, custom_id, provider, model, messages, temperature, max_tokens, callback_url, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                job.id,
                job.custom_id,
                job.provider,
                job.model,
                messages_json,
                job.temperature,
                job.max_tokens,
                job.callback_url,
                "queued",
                job.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get pending jobs for processing
    pub fn get_pending(&self, limit: usize) -> anyhow::Result<Vec<BatchJob>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, custom_id, provider, model, messages, temperature, max_tokens, callback_url, status, created_at, completed_at, response, error
             FROM batch_jobs
             WHERE status = 'queued'
             ORDER BY created_at ASC
             LIMIT ?1",
        )?;

        let jobs = stmt.query_map(rusqlite::params![limit as i64], |row| {
            let messages_json: String = row.get(4)?;
            let messages: Vec<ChatMessage> =
                serde_json::from_str(&messages_json).unwrap_or_default();
            let status_str: String = row.get(8)?;
            let status = match status_str.as_str() {
                "queued" => BatchStatus::Queued,
                "processing" => BatchStatus::Processing,
                "completed" => BatchStatus::Completed,
                "failed" => BatchStatus::Failed,
                _ => BatchStatus::Queued,
            };

            Ok(BatchJob {
                id: row.get(0)?,
                custom_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                messages,
                temperature: row.get(5)?,
                max_tokens: row.get(6)?,
                callback_url: row.get(7)?,
                status,
                created_at: row.get(9)?,
                completed_at: row.get(10)?,
                response: row.get(11)?,
                error: row.get(12)?,
            })
        })?;

        jobs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Update job status to processing
    pub fn mark_processing(&self, job_id: &str) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute(
            "UPDATE batch_jobs SET status = 'processing' WHERE id = ?1",
            rusqlite::params![job_id],
        )?;
        Ok(())
    }

    /// Mark job as completed with response
    pub fn mark_completed(&self, job_id: &str, response: &str) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE batch_jobs SET status = 'completed', completed_at = ?1, response = ?2 WHERE id = ?3",
            rusqlite::params![now, response, job_id],
        )?;
        Ok(())
    }

    /// Mark job as failed with error
    pub fn mark_failed(&self, job_id: &str, error: &str) -> anyhow::Result<()> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE batch_jobs SET status = 'failed', completed_at = ?1, error = ?2 WHERE id = ?3",
            rusqlite::params![now, error, job_id],
        )?;
        Ok(())
    }

    /// Get job by ID
    pub fn get_job(&self, job_id: &str) -> anyhow::Result<Option<BatchJob>> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, custom_id, provider, model, messages, temperature, max_tokens, callback_url, status, created_at, completed_at, response, error
             FROM batch_jobs WHERE id = ?1",
        )?;

        stmt.query_row(rusqlite::params![job_id], |row| {
            let messages_json: String = row.get(4)?;
            let messages: Vec<ChatMessage> =
                serde_json::from_str(&messages_json).unwrap_or_default();
            let status_str: String = row.get(8)?;
            let status = match status_str.as_str() {
                "queued" => BatchStatus::Queued,
                "processing" => BatchStatus::Processing,
                "completed" => BatchStatus::Completed,
                "failed" => BatchStatus::Failed,
                _ => BatchStatus::Queued,
            };

            Ok(BatchJob {
                id: row.get(0)?,
                custom_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                messages,
                temperature: row.get(5)?,
                max_tokens: row.get(6)?,
                callback_url: row.get(7)?,
                status,
                created_at: row.get(9)?,
                completed_at: row.get(10)?,
                response: row.get(11)?,
                error: row.get(12)?,
            })
        })
        .optional()
        .map_err(Into::into)
    }

    /// Get queue stats
    pub fn stats(&self) -> anyhow::Result<serde_json::Value> {
        let conn = rusqlite::Connection::open(&self.db_path)?;

        let queued: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_jobs WHERE status = 'queued'",
            [],
            |row| row.get(0),
        )?;

        let processing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_jobs WHERE status = 'processing'",
            [],
            |row| row.get(0),
        )?;

        let completed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_jobs WHERE status = 'completed'",
            [],
            |row| row.get(0),
        )?;

        let failed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_jobs WHERE status = 'failed'",
            [],
            |row| row.get(0),
        )?;

        Ok(json!({
            "queued": queued,
            "processing": processing,
            "completed": completed,
            "failed": failed,
            "total": queued + processing + completed + failed,
        }))
    }

    /// Cleanup old completed/failed jobs (retention in days)
    pub fn cleanup(&self, retention_days: u32) -> anyhow::Result<u64> {
        let conn = rusqlite::Connection::open(&self.db_path)?;
        let cutoff =
            (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).to_rfc3339();

        let deleted = conn.execute(
            "DELETE FROM batch_jobs WHERE status IN ('completed', 'failed') AND completed_at < ?1",
            rusqlite::params![cutoff],
        )?;

        Ok(deleted as u64)
    }
}

/// Request to submit a batch job
#[derive(Debug, Deserialize)]
pub struct BatchSubmitRequest {
    pub custom_id: Option<String>,
    pub provider: Option<String>,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub callback_url: Option<String>,
}

/// Create a batch job from request
pub fn create_batch_job(req: BatchSubmitRequest) -> BatchJob {
    BatchJob {
        id: uuid::Uuid::new_v4().to_string(),
        custom_id: req.custom_id,
        provider: req.provider.unwrap_or_else(|| "openai".to_string()),
        model: req.model,
        messages: req.messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        callback_url: req.callback_url,
        status: BatchStatus::Queued,
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        response: None,
        error: None,
    }
}
