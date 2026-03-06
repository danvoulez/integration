use anyhow::{anyhow, Result};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{self, Duration},
};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct SupabaseSyncConfig {
    pub url: String,
    pub service_role_key: String,
    pub tenant_id: String,
    pub app_id: String,
    pub user_id: Option<String>,
    pub realtime_enabled: bool,
    pub realtime_channel: String,
}

#[derive(Debug, Clone)]
pub struct Code247JobMirror {
    pub id: String,
    pub issue_id: String,
    pub status: String,
    pub payload: Value,
    pub retries: i32,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct Code247CheckpointMirror {
    pub job_id: String,
    pub stage: String,
    pub data: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Code247EventMirror {
    pub event_id: String,
    pub job_id: String,
    pub issue_id: Option<String>,
    pub stage: String,
    pub event_type: String,
    pub trace_id: String,
    pub outcome: String,
    pub retry_count: i32,
    pub fallback_used: bool,
    pub input: Option<String>,
    pub output: Option<String>,
    pub model_used: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
enum SupabaseSyncEvent {
    JobUpsert(Code247JobMirror),
    CheckpointUpsert(Code247CheckpointMirror),
    ExecutionEvent(Code247EventMirror),
}

#[derive(Clone)]
pub struct SupabaseSyncHandle {
    tx: mpsc::UnboundedSender<SupabaseSyncEvent>,
}

impl SupabaseSyncHandle {
    pub fn enqueue_job_upsert(&self, record: Code247JobMirror) {
        if let Err(err) = self.tx.send(SupabaseSyncEvent::JobUpsert(record)) {
            warn!(error=%err, "failed to enqueue code247 job supabase sync event");
        }
    }

    pub fn enqueue_checkpoint_upsert(&self, record: Code247CheckpointMirror) {
        if let Err(err) = self.tx.send(SupabaseSyncEvent::CheckpointUpsert(record)) {
            warn!(error=%err, "failed to enqueue code247 checkpoint supabase sync event");
        }
    }

    pub fn enqueue_execution_event(&self, record: Code247EventMirror) {
        if let Err(err) = self.tx.send(SupabaseSyncEvent::ExecutionEvent(record)) {
            warn!(error=%err, "failed to enqueue code247 execution supabase sync event");
        }
    }
}

pub fn spawn_sync_worker(
    config: SupabaseSyncConfig,
    mut shutdown: watch::Receiver<bool>,
) -> (SupabaseSyncHandle, JoinHandle<Result<()>>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<SupabaseSyncEvent>();
    let handle = SupabaseSyncHandle { tx };
    let join = tokio::spawn(async move {
        let client = Client::new();
        let mut events_table_supported = true;
        let mut flush_tick = time::interval(Duration::from_secs(30));

        info!(
            supabase_url=%config.url,
            tenant_id=%config.tenant_id,
            app_id=%config.app_id,
            realtime_enabled=config.realtime_enabled,
            realtime_channel=%config.realtime_channel,
            "supabase sync worker started"
        );

        loop {
            tokio::select! {
                _ = flush_tick.tick() => {}
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("supabase sync worker shutdown complete");
                        return Ok(());
                    }
                }
                maybe_event = rx.recv() => {
                    let Some(event) = maybe_event else {
                        info!("supabase sync queue closed");
                        return Ok(());
                    };
                    if let Err(err) = process_event(
                        &client,
                        &config,
                        event,
                        &mut events_table_supported,
                    ).await {
                        warn!(error=%err, "supabase sync event failed");
                    }
                }
            }
        }
    });
    (handle, join)
}

async fn process_event(
    client: &Client,
    config: &SupabaseSyncConfig,
    event: SupabaseSyncEvent,
    events_table_supported: &mut bool,
) -> Result<()> {
    match event {
        SupabaseSyncEvent::JobUpsert(job) => {
            let body = json!({
                "id": job.id,
                "tenant_id": config.tenant_id,
                "app_id": config.app_id,
                "user_id": config.user_id,
                "issue_id": job.issue_id,
                "status": job.status,
                "payload": job.payload,
                "retries": job.retries,
                "last_error": job.last_error,
                "created_at": job.created_at,
                "updated_at": job.updated_at,
            });
            postgrest_upsert(client, config, "code247_jobs", &body).await?;
            if config.realtime_enabled {
                broadcast_job_status(client, config, &job).await?;
            }
        }
        SupabaseSyncEvent::CheckpointUpsert(checkpoint) => {
            let body = json!({
                "job_id": checkpoint.job_id,
                "stage": checkpoint.stage,
                "data": checkpoint.data,
                "created_at": checkpoint.created_at,
            });
            postgrest_upsert(client, config, "code247_checkpoints", &body).await?;
        }
        SupabaseSyncEvent::ExecutionEvent(event) => {
            let metadata = json!({
                "event_type": event.event_type,
                "trace_id": event.trace_id,
                "parent_event_id": Value::Null,
                "outcome": event.outcome,
                "job_id": event.job_id,
                "issue_id": event.issue_id,
                "stage": event.stage,
                "model_used": event.model_used,
                "duration_ms": event.duration_ms,
                "retry_count": event.retry_count,
                "fallback_used": event.fallback_used,
            });
            emit_code247_fuel_event(client, config, &event, metadata).await?;

            if !*events_table_supported {
                return Ok(());
            }

            let body = json!({
                "event_id": event.event_id,
                "tenant_id": config.tenant_id,
                "app_id": config.app_id,
                "user_id": config.user_id,
                "job_id": event.job_id,
                "stage": event.stage,
                "event_type": event.event_type,
                "input": event.input,
                "output": event.output,
                "model_used": event.model_used,
                "duration_ms": event.duration_ms,
                "occurred_at": event.created_at,
                "metadata": json!({
                    "trace_id": event.trace_id,
                    "outcome": event.outcome,
                    "issue_id": event.issue_id,
                }),
            });
            if let Err(err) = postgrest_insert_ignore_duplicates(
                client,
                config,
                "code247_events",
                "event_id",
                &body,
            )
            .await
            {
                let text = err.to_string().to_ascii_lowercase();
                if text.contains("404")
                    || text.contains("code247_events")
                    || text.contains("relation")
                {
                    *events_table_supported = false;
                    warn!(
                        error=%err,
                        "code247_events table unavailable; disabling event sync until restart"
                    );
                    return Ok(());
                }
                return Err(err);
            }
        }
    }

    Ok(())
}

async fn postgrest_upsert(
    client: &Client,
    config: &SupabaseSyncConfig,
    table: &str,
    body: &Value,
) -> Result<()> {
    let resp = client
        .post(format!("{}/rest/v1/{}", config.url, table))
        .header("apikey", &config.service_role_key)
        .header(
            "Authorization",
            format!("Bearer {}", config.service_role_key),
        )
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=merge-duplicates,return=minimal")
        .json(body)
        .send()
        .await?;
    ensure_success(resp.status(), resp.text().await.unwrap_or_default(), table)
}

async fn postgrest_insert_ignore_duplicates(
    client: &Client,
    config: &SupabaseSyncConfig,
    table: &str,
    conflict_column: &str,
    body: &Value,
) -> Result<()> {
    let resp = client
        .post(format!(
            "{}/rest/v1/{}?on_conflict={}",
            config.url, table, conflict_column
        ))
        .header("apikey", &config.service_role_key)
        .header(
            "Authorization",
            format!("Bearer {}", config.service_role_key),
        )
        .header("Content-Type", "application/json")
        .header("Prefer", "resolution=ignore-duplicates,return=minimal")
        .json(body)
        .send()
        .await?;
    ensure_success(resp.status(), resp.text().await.unwrap_or_default(), table)
}

async fn emit_code247_fuel_event(
    client: &Client,
    config: &SupabaseSyncConfig,
    event: &Code247EventMirror,
    metadata: Value,
) -> Result<()> {
    let Some(user_id) = config.user_id.as_ref() else {
        warn!(
            event_id=%event.event_id,
            "CODE247_SUPABASE_USER_ID ausente; fuel_events para code247 será ignorado"
        );
        return Ok(());
    };

    let idempotency_key = format!("code247:fuel:{}", event.event_id);
    if let Err(reason) = validate_fuel_metadata(&metadata) {
        warn!(
            reason=%reason,
            event_id=%event.event_id,
            event_type=%event.event_type,
            trace_id=%event.trace_id,
            "fuel.emit.invalid"
        );
        return Err(anyhow!("invalid fuel metadata: {reason}"));
    }

    let fuel_row = json!({
        "event_id": format!("fuel:{}", event.event_id),
        "idempotency_key": idempotency_key,
        "tenant_id": config.tenant_id,
        "app_id": config.app_id,
        "user_id": user_id,
        "units": 1,
        "unit_type": "code_event",
        "occurred_at": event.created_at,
        "source": "code247:pipeline",
        "metadata": metadata,
    });

    postgrest_insert_ignore_duplicates(client, config, "fuel_events", "idempotency_key", &fuel_row)
        .await
}

async fn broadcast_job_status(
    client: &Client,
    config: &SupabaseSyncConfig,
    job: &Code247JobMirror,
) -> Result<()> {
    let channel = config
        .realtime_channel
        .replace("{tenant_id}", config.tenant_id.as_str());
    let stage = stage_from_status(&job.status);
    let body = json!({
        "channel": channel,
        "event": "job_status",
        "payload": {
            "job_id": job.id,
            "status": job.status,
            "stage": stage,
            "error": job.last_error,
            "timestamp": job.updated_at,
        }
    });

    let resp = client
        .post(format!("{}/realtime/v1/api/broadcast", config.url))
        .header("apikey", &config.service_role_key)
        .header(
            "Authorization",
            format!("Bearer {}", config.service_role_key),
        )
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    ensure_success(
        resp.status(),
        resp.text().await.unwrap_or_default(),
        "realtime:broadcast",
    )
}

fn stage_from_status(status: &str) -> &'static str {
    match status {
        "PENDING" => "pending",
        "PLANNING" => "planning",
        "CODING" => "coding",
        "REVIEWING" => "reviewing",
        "VALIDATING" => "validating",
        "COMMITTING" => "committing",
        "FAILED" => "failed",
        "DONE" => "done",
        _ => "unknown",
    }
}

fn ensure_success(status: StatusCode, body: String, target: &str) -> Result<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(anyhow!(
        "supabase sync failed for {target}: status={} body={}",
        status,
        body
    ))
}

fn validate_fuel_metadata(metadata: &Value) -> std::result::Result<(), String> {
    let Some(map) = metadata.as_object() else {
        return Err("metadata must be a JSON object".into());
    };

    for key in ["event_type", "trace_id", "outcome"] {
        let Some(value) = map.get(key).and_then(Value::as_str) else {
            return Err(format!("missing or invalid metadata.{key}"));
        };
        if value.trim().is_empty() {
            return Err(format!("metadata.{key} cannot be empty"));
        }
    }

    if !map.contains_key("parent_event_id") {
        return Err("missing metadata.parent_event_id".into());
    }
    if !map
        .get("parent_event_id")
        .map(|value| value.is_null() || value.is_string())
        .unwrap_or(false)
    {
        return Err("metadata.parent_event_id must be string|null".into());
    }

    Ok(())
}
