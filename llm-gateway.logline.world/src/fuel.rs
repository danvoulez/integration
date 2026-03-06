use crate::config::{Config, SupabaseConfig};
use crate::types::FuelDelta;
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub fn maybe_redact_upstream_url(config: &Config, upstream_url: &str) -> String {
    if config.security.expose_upstream_url {
        upstream_url.to_string()
    } else {
        "redacted".into()
    }
}

fn default_fuel_db_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".llm-gateway").join("fuel.db"))
        .unwrap_or_else(|| PathBuf::from("fuel.db"))
}

fn init_fuel_db(path: &Path) -> anyhow::Result<()> {
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

fn upsert_daily_fuel(path: &str, delta: FuelDelta) -> anyhow::Result<()> {
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

/// Emit fuel event to Supabase for per-caller billing.
/// 
/// This inserts into the fuel_events table, which is an append-only ledger
/// used for monthly billing reports. Each event has an idempotency key
/// to prevent duplicate billing.
async fn emit_fuel_to_supabase(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    identity: &ClientIdentity,
    delta: &FuelDelta,
    metadata: serde_json::Value,
) -> anyhow::Result<()> {
    // Skip if not billable to Supabase
    if !identity.is_supabase_billable() {
        return Ok(());
    }
    
    let url = config.url.as_deref().ok_or_else(|| anyhow::anyhow!("Supabase URL not configured"))?;
    let key = config.service_role_key.as_deref().ok_or_else(|| anyhow::anyhow!("Supabase service_role_key not configured"))?;
    
    let tenant_id = identity.tenant_id.as_ref().unwrap();
    let app_id = identity.app_id.as_ref().unwrap();
    let user_id = identity.user_id.as_ref().map(|s| s.as_str()).unwrap_or(tenant_id);
    
    // Generate idempotency key from request fingerprint
    let idempotency_key = format!(
        "llm-gateway:{}:{}:{}",
        app_id,
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        uuid::Uuid::new_v4()
    );
    
    // Calculate total tokens as the unit for billing
    let total_tokens = delta.prompt_tokens + delta.completion_tokens;
    if total_tokens == 0 {
        return Ok(());
    }
    
    let fuel_event = json!({
        "idempotency_key": idempotency_key,
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "units": total_tokens,
        "unit_type": "llm_tokens",
        "source": identity.fuel_source(),
        "metadata": metadata,
    });
    
    let resp = client
        .post(format!("{}/rest/v1/fuel_events", url))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&fuel_event)
        .send()
        .await?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Failed to emit fuel event to Supabase");
        return Err(anyhow::anyhow!("Supabase fuel insert failed: {} - {}", status, body));
    }
    
    Ok(())
}

/// Log LLM request telemetry to Supabase.
/// 
/// This inserts into the llm_requests table for analytics and debugging.
/// Unlike fuel_events, this is not used for billing - it stores detailed
/// request metadata like provider, model, latency, and error messages.
async fn log_llm_request_to_supabase(
    client: &reqwest::Client,
    config: &SupabaseConfig,
    identity: &ClientIdentity,
    provider: &str,
    model: &str,
    mode: &str,
    input_tokens: u32,
    output_tokens: u32,
    latency_ms: u32,
    success: bool,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    // Skip if not billable (no tenant/app context)
    if !identity.is_supabase_billable() {
        return Ok(());
    }
    
    let url = config.url.as_deref().ok_or_else(|| anyhow::anyhow!("Supabase URL not configured"))?;
    let key = config.service_role_key.as_deref().ok_or_else(|| anyhow::anyhow!("Supabase service_role_key not configured"))?;
    
    let tenant_id = identity.tenant_id.as_ref().unwrap();
    let app_id = identity.app_id.as_ref().unwrap();
    let user_id = identity.user_id.as_ref();
    
    let request_log = json!({
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "provider": provider,
        "model": model,
        "mode": mode,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "latency_ms": latency_ms,
        "success": success,
        "error_message": error_message,
    });
    
    let resp = client
        .post(format!("{}/rest/v1/llm_requests", url))
        .header("apikey", key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .json(&request_log)
        .send()
        .await?;
    
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, body = %body, "Failed to log LLM request to Supabase");
        // Don't return error - logging failure shouldn't block the request
    }
    
    Ok(())
}

fn read_daily_fuel(path: &str, limit: i64) -> anyhow::Result<Vec<serde_json::Value>> {
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

