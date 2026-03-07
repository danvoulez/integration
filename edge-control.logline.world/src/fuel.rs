use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::{auth::AuthContext, resilience::send_with_resilience, AppState};

#[derive(Clone)]
pub struct FuelEventPayload {
    pub event_id: Option<String>,
    pub event_type: String,
    pub trace_id: String,
    pub parent_event_id: Option<String>,
    pub outcome: String,
    pub reason_codes: Vec<String>,
    pub metadata_extra: Value,
}

pub async fn emit_event(
    state: &AppState,
    auth: &AuthContext,
    payload: FuelEventPayload,
) -> Result<String> {
    let Some(url) = state.config.supabase_url.as_deref() else {
        return Err(anyhow!("SUPABASE_URL not configured"));
    };
    let Some(service_role_key) = state.config.supabase_service_role_key.as_deref() else {
        return Err(anyhow!("SUPABASE_SERVICE_ROLE_KEY not configured"));
    };

    let tenant_id = auth
        .tenant_id
        .clone()
        .or_else(|| state.config.default_tenant_id.clone())
        .ok_or_else(|| anyhow!("missing tenant_id for fuel emission"))?;
    let app_id = auth
        .app_id
        .clone()
        .or_else(|| state.config.default_app_id.clone())
        .unwrap_or_else(|| "edge-control".into());
    let user_id = if auth.subject.trim().is_empty() {
        state
            .config
            .default_user_id
            .clone()
            .unwrap_or_else(|| "edge-control-system".into())
    } else {
        auth.subject.clone()
    };

    let event_id = payload
        .event_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let event_type = payload.event_type.clone();
    let trace_id = payload.trace_id.clone();
    let outcome = payload.outcome.clone();
    let reason_codes = payload.reason_codes.clone();
    let parent_event_id = payload.parent_event_id.clone();
    let metadata_extra = payload.metadata_extra.clone();

    let idempotency_key = format!("edge-control:{}:{}:{}", event_type, trace_id, event_id);

    let mut metadata = json!({
        "event_type": event_type,
        "trace_id": trace_id,
        "source": "edge-control",
        "actor_kind": if auth.role.as_deref() == Some("service") { "service" } else { "human" },
        "outcome": outcome,
        "reason_codes": reason_codes,
    });

    metadata["parent_event_id"] = parent_event_id.map(Value::String).unwrap_or(Value::Null);

    if let Some(extra) = metadata_extra.as_object() {
        for (k, v) in extra {
            metadata[k] = v.clone();
        }
    }

    if let Err(reason) = validate_fuel_metadata(&metadata) {
        warn!(
            reason=%reason,
            event_type=%payload.event_type,
            trace_id=%payload.trace_id,
            "fuel.emit.invalid"
        );
        return Err(anyhow!("invalid fuel metadata: {reason}"));
    }

    let fuel_row = json!({
        "event_id": event_id,
        "idempotency_key": idempotency_key,
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "units": 1,
        "unit_type": "api_call",
        "source": "edge-control",
        "metadata": metadata
    });

    let endpoint = format!("{url}/rest/v1/fuel_events");
    let service_role_key = service_role_key.to_string();
    let fuel_row_clone = fuel_row.clone();
    let response = send_with_resilience(
        &state.config,
        &state.circuit_breakers,
        "supabase.fuel_events",
        || {
            state
                .http_client
                .post(&endpoint)
                .header("apikey", &service_role_key)
                .header("Authorization", format!("Bearer {service_role_key}"))
                .header("Content-Type", "application/json")
                .header("Prefer", "return=minimal")
                .json(&fuel_row_clone)
        },
    )
    .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        warn!(%status, %body, "fuel emission rejected");
        return Err(anyhow!("fuel insert failed: {status}"));
    }

    if let Err(err) = emit_obs_event(state, &payload, &event_id).await {
        warn!(error=%err, event_id=%event_id, "obs-api ingest mirror failed");
    }

    Ok(event_id)
}

async fn emit_obs_event(
    state: &AppState,
    payload: &FuelEventPayload,
    event_id: &str,
) -> Result<()> {
    let Some(base_url) = state.config.obs_api_base_url.as_deref() else {
        return Ok(());
    };

    let occurred_at = chrono::Utc::now().to_rfc3339();
    let normalized_event_id = if Uuid::parse_str(event_id).is_ok() {
        event_id.to_string()
    } else {
        Uuid::new_v4().to_string()
    };

    let mut payload_object = payload
        .metadata_extra
        .as_object()
        .cloned()
        .unwrap_or_default();

    if normalized_event_id != event_id {
        payload_object.insert(
            "original_event_id".into(),
            Value::String(event_id.to_string()),
        );
    }

    let intention_id = read_string(&payload_object, "intention_id");
    let run_id = read_string(&payload_object, "run_id");
    let issue_id = read_string(&payload_object, "issue_id");
    let pr_id = read_string(&payload_object, "pr_id");
    let deploy_id = read_string(&payload_object, "deploy_id");

    let event_body = json!({
        "event_id": normalized_event_id,
        "event_type": payload.event_type,
        "occurred_at": occurred_at,
        "source": "edge-control",
        "request_id": payload.trace_id,
        "trace_id": payload.trace_id,
        "parent_event_id": payload.parent_event_id,
        "intention_id": intention_id,
        "run_id": run_id,
        "issue_id": issue_id,
        "pr_id": pr_id,
        "deploy_id": deploy_id,
        "payload": Value::Object(payload_object),
    });

    let url = format!("{}/api/v1/events/ingest", base_url.trim_end_matches('/'));
    let token = state.config.obs_api_token.clone();
    let event_body_clone = event_body.clone();
    let response =
        send_with_resilience(&state.config, &state.circuit_breakers, "obs.ingest", || {
            let mut req = state.http_client.post(&url).json(&event_body_clone);
            if let Some(token) = token.as_deref() {
                req = req.bearer_auth(token);
            }
            req
        })
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("obs ingest failed: {status} {body}"));
    }

    Ok(())
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

fn read_string(payload: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate_fuel_metadata;

    #[test]
    fn fuel_metadata_allows_root_event_with_null_parent() {
        let metadata = json!({
            "event_type": "pr.risk.opinion_emitted",
            "trace_id": "trace-123",
            "outcome": "emitted",
            "parent_event_id": null
        });

        assert!(validate_fuel_metadata(&metadata).is_ok());
    }
}
