//! Database operations for fuel tracking, QC sampling, and client management.

use crate::config::QcPolicy;
use crate::types::{ChatRequest, FuelDelta};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Client identity for authentication and billing attribution.
///
/// For API key auth (legacy): client_id + app_name from local SQLite.
/// For Supabase JWT auth: tenant_id + app_id + user_id from token claims.
#[derive(Clone, Debug)]
pub struct ClientIdentity {
    /// Legacy: local client ID from SQLite
    pub client_id: i64,
    /// Legacy: app name from SQLite
    pub app_name: String,
    /// Supabase: tenant UUID (from JWT sub or claim)
    pub tenant_id: Option<String>,
    /// Supabase: app UUID (from JWT claim)
    pub app_id: Option<String>,
    /// Supabase: user UUID (from JWT claim, if present)
    pub user_id: Option<String>,
    /// Calling app (from x-calling-app header) - which ecosystem app originated the request
    pub calling_app: Option<String>,
}

impl ClientIdentity {
    /// Create legacy identity from API key lookup
    pub fn from_api_key(client_id: i64, app_name: String) -> Self {
        Self {
            client_id,
            app_name,
            tenant_id: None,
            app_id: None,
            user_id: None,
            calling_app: None,
        }
    }

    /// Create identity from Supabase JWT claims
    pub fn from_supabase_jwt(tenant_id: String, app_id: String, user_id: Option<String>) -> Self {
        Self {
            client_id: 0,
            app_name: format!("supa:{}", &app_id[..8.min(app_id.len())]),
            tenant_id: Some(tenant_id),
            app_id: Some(app_id),
            user_id,
            calling_app: None,
        }
    }

    /// Create identity from service token (long-lived app-to-app JWT)
    /// Service tokens don't have user_id - the app itself is the identity
    pub fn from_service_token(tenant_id: String, app_id: String) -> Self {
        Self {
            client_id: 0,
            app_name: format!("svc:{}", &app_id[..8.min(app_id.len())]),
            tenant_id: Some(tenant_id),
            app_id: Some(app_id),
            user_id: None, // Service tokens have no user - they act as the app
            calling_app: None,
        }
    }

    /// Create admin identity for gateway's own API key
    pub fn admin() -> Self {
        Self {
            client_id: 0,
            app_name: "admin".into(),
            tenant_id: None,
            app_id: None,
            user_id: None,
            calling_app: None,
        }
    }

    /// Set the calling app (from x-calling-app header)
    pub fn with_calling_app(mut self, calling_app: Option<String>) -> Self {
        self.calling_app = calling_app;
        self
    }

    /// Get the source for fuel events (calling_app if present, otherwise "direct")
    pub fn fuel_source(&self) -> &str {
        self.calling_app.as_deref().unwrap_or("direct")
    }

    /// Check if this identity can be billed to Supabase
    pub fn is_supabase_billable(&self) -> bool {
        self.tenant_id.is_some() && self.app_id.is_some()
    }
}

pub struct QcSampleRow {
    pub sample_key: String,
    pub mode_used: String,
    pub task_class: String,
    pub provider: String,
    pub model: String,
    pub is_stream: bool,
    pub success: bool,
    pub latency_ms: u64,
    pub error_message: Option<String>,
    pub prompt_excerpt: String,
    pub response_excerpt: String,
    pub decision_path: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct QcSamplesQuery {
    pub day: Option<String>,
    pub provider: Option<String>,
    pub success: Option<bool>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ClientUsageQuery {
    pub day: Option<String>,
    pub app_name: Option<String>,
    pub mode: Option<String>,
    pub limit: Option<u32>,
}

pub fn init_fuel_db(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_fuel (
            day TEXT PRIMARY KEY,
            calls_total INTEGER NOT NULL DEFAULT 0,
            calls_success INTEGER NOT NULL DEFAULT 0,
            calls_failed INTEGER NOT NULL DEFAULT 0,
            calls_stream INTEGER NOT NULL DEFAULT 0,
            calls_non_stream INTEGER NOT NULL DEFAULT 0,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS qc_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sampled_at TEXT NOT NULL,
            day TEXT NOT NULL,
            sample_key TEXT NOT NULL,
            mode_used TEXT NOT NULL,
            task_class TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            is_stream INTEGER NOT NULL DEFAULT 0,
            success INTEGER NOT NULL DEFAULT 0,
            latency_ms INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            prompt_excerpt TEXT NOT NULL,
            response_excerpt TEXT NOT NULL,
            decision_path TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS api_clients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name TEXT NOT NULL UNIQUE,
            api_key TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            last_used_at TEXT
        );
        CREATE TABLE IF NOT EXISTS daily_client_usage (
            day TEXT NOT NULL,
            client_id INTEGER NOT NULL,
            app_name TEXT NOT NULL,
            mode_used TEXT NOT NULL,
            calls_total INTEGER NOT NULL DEFAULT 0,
            calls_success INTEGER NOT NULL DEFAULT 0,
            calls_failed INTEGER NOT NULL DEFAULT 0,
            calls_stream INTEGER NOT NULL DEFAULT 0,
            calls_non_stream INTEGER NOT NULL DEFAULT 0,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (day, client_id, mode_used)
        );",
    )?;
    Ok(())
}

pub fn upsert_daily_fuel(path: &str, delta: FuelDelta) -> anyhow::Result<()> {
    if delta.calls_total == 0
        && delta.calls_success == 0
        && delta.calls_failed == 0
        && delta.calls_stream == 0
        && delta.calls_non_stream == 0
        && delta.prompt_tokens == 0
        && delta.completion_tokens == 0
    {
        return Ok(());
    }

    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let conn = rusqlite::Connection::open(path)?;
    conn.execute(
        "INSERT INTO daily_fuel (
            day, calls_total, calls_success, calls_failed, calls_stream, calls_non_stream,
            prompt_tokens, completion_tokens, total_tokens, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(day) DO UPDATE SET
            calls_total = calls_total + excluded.calls_total,
            calls_success = calls_success + excluded.calls_success,
            calls_failed = calls_failed + excluded.calls_failed,
            calls_stream = calls_stream + excluded.calls_stream,
            calls_non_stream = calls_non_stream + excluded.calls_non_stream,
            prompt_tokens = prompt_tokens + excluded.prompt_tokens,
            completion_tokens = completion_tokens + excluded.completion_tokens,
            total_tokens = total_tokens + excluded.total_tokens,
            updated_at = excluded.updated_at",
        rusqlite::params![
            day,
            delta.calls_total as i64,
            delta.calls_success as i64,
            delta.calls_failed as i64,
            delta.calls_stream as i64,
            delta.calls_non_stream as i64,
            delta.prompt_tokens as i64,
            delta.completion_tokens as i64,
            (delta.prompt_tokens + delta.completion_tokens) as i64,
            now
        ],
    )?;
    Ok(())
}

pub fn read_daily_fuel(path: &str, limit: i64) -> anyhow::Result<Vec<serde_json::Value>> {
    let conn = rusqlite::Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT day, calls_total, calls_success, calls_failed, calls_stream, calls_non_stream,
                prompt_tokens, completion_tokens, total_tokens, updated_at
         FROM daily_fuel
         ORDER BY day DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| {
        Ok(json!({
            "day": row.get::<_, String>(0)?,
            "calls_total": row.get::<_, i64>(1)?,
            "calls_success": row.get::<_, i64>(2)?,
            "calls_failed": row.get::<_, i64>(3)?,
            "calls_stream": row.get::<_, i64>(4)?,
            "calls_non_stream": row.get::<_, i64>(5)?,
            "prompt_tokens": row.get::<_, i64>(6)?,
            "completion_tokens": row.get::<_, i64>(7)?,
            "total_tokens": row.get::<_, i64>(8)?,
            "updated_at": row.get::<_, String>(9)?
        }))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn should_sample_qc(req: &ChatRequest, policy: &QcPolicy) -> bool {
    if !policy.enabled || policy.sampling_rate <= 0.0 {
        return false;
    }
    if req.stream.unwrap_or(false) && !policy.include_stream {
        return false;
    }
    if policy.sampling_rate >= 1.0 {
        return true;
    }

    let mut hasher = DefaultHasher::new();
    req.model.hash(&mut hasher);
    req.mode.hash(&mut hasher);
    req.task_hint.hash(&mut hasher);
    req.stream.unwrap_or(false).hash(&mut hasher);
    for m in &req.messages {
        m.role.hash(&mut hasher);
        m.content.hash(&mut hasher);
    }
    let bucket = hasher.finish() % 10_000;
    (bucket as f64) < (policy.sampling_rate * 10_000.0)
}

pub fn qc_request_key(req: &ChatRequest) -> String {
    let mut hasher = DefaultHasher::new();
    req.model.hash(&mut hasher);
    req.mode.hash(&mut hasher);
    req.task_hint.hash(&mut hasher);
    req.stream.unwrap_or(false).hash(&mut hasher);
    for m in &req.messages {
        m.role.hash(&mut hasher);
        m.content.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

pub fn redact_qc_text(input: &str, max_chars: usize) -> String {
    let mut out = input.replace('\n', " ");
    for prefix in ["sk-", "AIza", "Bearer "] {
        while let Some(pos) = out.find(prefix) {
            let end = (pos + 48).min(out.len());
            out.replace_range(pos..end, "[REDACTED_SECRET]");
        }
    }
    if out.len() > max_chars {
        out.truncate(max_chars);
    }
    out
}

pub fn insert_qc_sample(path: &str, row: &QcSampleRow, retention_days: u32) -> anyhow::Result<()> {
    let conn = rusqlite::Connection::open(path)?;
    let sampled_at = chrono::Utc::now().to_rfc3339();
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    conn.execute(
        "INSERT INTO qc_samples (
            sampled_at, day, sample_key, mode_used, task_class, provider, model,
            is_stream, success, latency_ms, error_message, prompt_excerpt, response_excerpt, decision_path
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            sampled_at,
            day,
            row.sample_key,
            row.mode_used,
            row.task_class,
            row.provider,
            row.model,
            if row.is_stream { 1 } else { 0 },
            if row.success { 1 } else { 0 },
            row.latency_ms as i64,
            row.error_message,
            row.prompt_excerpt,
            row.response_excerpt,
            row.decision_path,
        ],
    )?;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days as i64))
        .format("%Y-%m-%d")
        .to_string();
    conn.execute(
        "DELETE FROM qc_samples WHERE day < ?1",
        rusqlite::params![cutoff],
    )?;
    Ok(())
}

pub fn create_api_client(path: &str, app_name: &str) -> anyhow::Result<(i64, String)> {
    let app = app_name.trim();
    if app.is_empty() {
        anyhow::bail!("app_name cannot be empty");
    }
    let api_key = format!(
        "lgw_{}_{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let created_at = chrono::Utc::now().to_rfc3339();
    let conn = rusqlite::Connection::open(path)?;
    conn.execute(
        "INSERT INTO api_clients (app_name, api_key, status, created_at) VALUES (?1, ?2, 'active', ?3)",
        rusqlite::params![app, api_key, created_at],
    )?;
    let id = conn.last_insert_rowid();
    Ok((id, api_key))
}

pub fn resolve_client_by_api_key(
    path: &str,
    api_key: &str,
) -> anyhow::Result<Option<(i64, String)>> {
    let conn = rusqlite::Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT id, app_name FROM api_clients WHERE api_key = ?1 AND status = 'active' LIMIT 1",
    )?;
    let row = stmt
        .query_row(rusqlite::params![api_key], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .optional()?;
    if let Some((id, app_name)) = &row {
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE api_clients SET last_used_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        );
        return Ok(Some((*id, app_name.clone())));
    }
    Ok(None)
}

pub fn upsert_api_client_by_app(
    path: &str,
    app_name: &str,
    rotate: bool,
) -> anyhow::Result<(i64, String, bool)> {
    let app = app_name.trim();
    if app.is_empty() {
        anyhow::bail!("app_name cannot be empty");
    }
    let conn = rusqlite::Connection::open(path)?;
    let existing: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, api_key FROM api_clients WHERE app_name = ?1 AND status = 'active' LIMIT 1",
            rusqlite::params![app],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((id, key)) = existing {
        if rotate {
            let new_key = format!(
                "lgw_{}_{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            );
            conn.execute(
                "UPDATE api_clients SET api_key = ?1 WHERE id = ?2",
                rusqlite::params![new_key, id],
            )?;
            return Ok((id, new_key, true));
        }
        return Ok((id, key, false));
    }

    let (id, key) = create_api_client(path, app)?;
    Ok((id, key, true))
}

pub fn upsert_daily_client_usage(
    path: &str,
    client: &ClientIdentity,
    mode_used: &str,
    delta: FuelDelta,
) -> anyhow::Result<()> {
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let conn = rusqlite::Connection::open(path)?;
    conn.execute(
        "INSERT INTO daily_client_usage (
            day, client_id, app_name, mode_used, calls_total, calls_success, calls_failed,
            calls_stream, calls_non_stream, prompt_tokens, completion_tokens, total_tokens, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(day, client_id, mode_used) DO UPDATE SET
            calls_total = calls_total + excluded.calls_total,
            calls_success = calls_success + excluded.calls_success,
            calls_failed = calls_failed + excluded.calls_failed,
            calls_stream = calls_stream + excluded.calls_stream,
            calls_non_stream = calls_non_stream + excluded.calls_non_stream,
            prompt_tokens = prompt_tokens + excluded.prompt_tokens,
            completion_tokens = completion_tokens + excluded.completion_tokens,
            total_tokens = total_tokens + excluded.total_tokens,
            updated_at = excluded.updated_at",
        rusqlite::params![
            day,
            client.client_id,
            client.app_name,
            mode_used,
            delta.calls_total as i64,
            delta.calls_success as i64,
            delta.calls_failed as i64,
            delta.calls_stream as i64,
            delta.calls_non_stream as i64,
            delta.prompt_tokens as i64,
            delta.completion_tokens as i64,
            (delta.prompt_tokens + delta.completion_tokens) as i64,
            now
        ],
    )?;
    Ok(())
}

pub fn read_daily_client_usage(
    path: &str,
    q: &ClientUsageQuery,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let conn = rusqlite::Connection::open(path)?;
    let limit = q.limit.unwrap_or(200).clamp(1, 1000) as i64;
    let mut stmt = conn.prepare(
        "SELECT day, client_id, app_name, mode_used, calls_total, calls_success, calls_failed,
                calls_stream, calls_non_stream, prompt_tokens, completion_tokens, total_tokens, updated_at
         FROM daily_client_usage
         WHERE (?1 IS NULL OR day = ?1)
           AND (?2 IS NULL OR app_name = ?2)
           AND (?3 IS NULL OR mode_used = ?3)
         ORDER BY day DESC, app_name ASC, mode_used ASC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![
            q.day.as_deref(),
            q.app_name.as_deref(),
            q.mode.as_deref(),
            limit
        ],
        |row| {
            Ok(json!({
                "day": row.get::<_, String>(0)?,
                "client_id": row.get::<_, i64>(1)?,
                "app_name": row.get::<_, String>(2)?,
                "mode_used": row.get::<_, String>(3)?,
                "calls_total": row.get::<_, i64>(4)?,
                "calls_success": row.get::<_, i64>(5)?,
                "calls_failed": row.get::<_, i64>(6)?,
                "calls_stream": row.get::<_, i64>(7)?,
                "calls_non_stream": row.get::<_, i64>(8)?,
                "prompt_tokens": row.get::<_, i64>(9)?,
                "completion_tokens": row.get::<_, i64>(10)?,
                "total_tokens": row.get::<_, i64>(11)?,
                "updated_at": row.get::<_, String>(12)?
            }))
        },
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn read_qc_samples(path: &str, q: &QcSamplesQuery) -> anyhow::Result<Vec<serde_json::Value>> {
    let conn = rusqlite::Connection::open(path)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 500) as i64;
    let success_int = q.success.map(|v| if v { 1_i64 } else { 0_i64 });
    let mut stmt = conn.prepare(
        "SELECT sampled_at, day, sample_key, mode_used, task_class, provider, model,
                is_stream, success, latency_ms, error_message, prompt_excerpt, response_excerpt, decision_path
         FROM qc_samples
         WHERE (?1 IS NULL OR day = ?1)
           AND (?2 IS NULL OR provider = ?2)
           AND (?3 IS NULL OR success = ?3)
         ORDER BY sampled_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        rusqlite::params![q.day.as_deref(), q.provider.as_deref(), success_int, limit],
        |row| {
            Ok(json!({
                "sampled_at": row.get::<_, String>(0)?,
                "day": row.get::<_, String>(1)?,
                "sample_key": row.get::<_, String>(2)?,
                "mode_used": row.get::<_, String>(3)?,
                "task_class": row.get::<_, String>(4)?,
                "provider": row.get::<_, String>(5)?,
                "model": row.get::<_, String>(6)?,
                "is_stream": row.get::<_, i64>(7)? == 1,
                "success": row.get::<_, i64>(8)? == 1,
                "latency_ms": row.get::<_, i64>(9)?,
                "error_message": row.get::<_, Option<String>>(10)?,
                "prompt_excerpt": row.get::<_, String>(11)?,
                "response_excerpt": row.get::<_, String>(12)?,
                "decision_path": row.get::<_, String>(13)?
            }))
        },
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
