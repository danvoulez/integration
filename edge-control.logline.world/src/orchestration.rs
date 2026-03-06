use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct HumanCheckpoint {
    pub checkpoint: String,
    pub confirmed_by: String,
    #[serde(default)]
    pub confirmed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrchestrateIntentionRequest {
    pub workspace: String,
    pub project: String,
    pub intention_id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default)]
    pub ci_target: Option<String>,
    pub checkpoint: HumanCheckpoint,
}

#[derive(Debug, Deserialize)]
pub struct OrchestrateGitHubEventRequest {
    pub workspace: String,
    pub project: String,
    pub intention_id: String,
    pub event_type: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub pr_url: Option<String>,
    #[serde(default)]
    pub ci_url: Option<String>,
    #[serde(default)]
    pub deploy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrchestrateRollbackRequest {
    pub workspace: String,
    pub project: String,
    pub intention_id: String,
    pub reason: String,
    pub strategy: String,
    #[serde(default)]
    pub rollback_url: Option<String>,
    pub checkpoint: HumanCheckpoint,
}

#[derive(Debug, Serialize)]
pub struct OrchestrateResponse {
    pub request_id: String,
    pub output_schema: &'static str,
    pub code247_request_id: String,
    pub queue_id: Option<String>,
    pub issue_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct Code247IntakeResponse {
    pub request_id: String,
    pub linear: Code247LinearBlock,
    pub ci: Code247CiBlock,
}

#[derive(Debug, Deserialize)]
pub struct Code247LinearBlock {
    pub intentions: Vec<Code247LinearLink>,
}

#[derive(Debug, Deserialize)]
pub struct Code247LinearLink {
    pub id: String,
    pub issue_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Code247CiBlock {
    #[allow(dead_code)]
    pub jobs: Vec<String>,
    pub queue_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Code247SyncResponse {
    pub request_id: String,
    pub synced: Vec<Code247SyncItem>,
    #[serde(default)]
    pub errors: Vec<Code247SyncError>,
}

#[derive(Debug, Deserialize)]
pub struct Code247SyncItem {
    pub intention_id: String,
    pub issue_id: String,
    pub moved_to_done: bool,
}

#[derive(Debug, Deserialize)]
pub struct Code247SyncError {
    pub code: String,
    pub message: String,
}

pub fn ensure_yes_human_1(cp: &HumanCheckpoint) -> Result<()> {
    if cp.checkpoint != "YES_HUMAN_1" {
        return Err(anyhow!("checkpoint must be YES_HUMAN_1"));
    }
    if cp.confirmed_by.trim().is_empty() {
        return Err(anyhow!("confirmed_by is required"));
    }
    Ok(())
}

pub fn ensure_yes_human_2(cp: &HumanCheckpoint) -> Result<()> {
    if cp.checkpoint != "YES_HUMAN_2" {
        return Err(anyhow!("checkpoint must be YES_HUMAN_2"));
    }
    if cp.confirmed_by.trim().is_empty() {
        return Err(anyhow!("confirmed_by is required"));
    }
    Ok(())
}

pub async fn call_code247_intentions(
    state: &AppState,
    request: &OrchestrateIntentionRequest,
) -> Result<Code247IntakeResponse> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    let payload = json!({
        "manifest": {
            "workspace": request.workspace,
            "project": request.project,
            "updated_at": updated_at,
            "intentions": [
                {
                    "id": request.intention_id,
                    "title": request.title,
                    "type": "feature",
                    "scope": request.description,
                    "priority": request.priority.clone().unwrap_or_else(|| "medium".into()),
                    "tasks": [
                        {
                            "description": request.description,
                            "owner": "code247",
                            "gate": "execution"
                        }
                    ]
                }
            ]
        },
        "source": "edge-control.intention-confirmed",
        "revision": request.revision,
        "ci_target": request.ci_target
    });

    let url = format!(
        "{}/intentions",
        state.config.code247_base_url.trim_end_matches('/')
    );

    let mut builder = state.http_client.post(url).json(&payload);
    if let Some(token) = state.config.code247_intentions_token.as_deref() {
        builder = builder.bearer_auth(token);
    }

    let response = builder.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("code247 /intentions failed: {status} {body}"));
    }

    let parsed = response.json::<Code247IntakeResponse>().await?;
    Ok(parsed)
}

pub async fn call_code247_sync(
    state: &AppState,
    workspace: &str,
    project: &str,
    intention_id: &str,
    status: &str,
    summary: &str,
    evidence: Value,
    set_done_on_success: bool,
) -> Result<Code247SyncResponse> {
    let payload = json!({
        "workspace": workspace,
        "project": project,
        "results": [
            {
                "intention_id": intention_id,
                "status": status,
                "summary": summary,
                "evidence": evidence,
                "set_done_on_success": set_done_on_success
            }
        ]
    });

    let url = format!(
        "{}/intentions/sync",
        state.config.code247_base_url.trim_end_matches('/')
    );
    let mut builder = state.http_client.post(url).json(&payload);
    if let Some(token) = state.config.code247_intentions_token.as_deref() {
        builder = builder.bearer_auth(token);
    }

    let response = builder.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("code247 /intentions/sync failed: {status} {body}"));
    }

    let parsed = response.json::<Code247SyncResponse>().await?;
    if let Some(err) = parsed.errors.first() {
        return Err(anyhow!("code247 sync error: {} {}", err.code, err.message));
    }
    Ok(parsed)
}
