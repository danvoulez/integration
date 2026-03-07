use anyhow::{anyhow, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{resilience::send_with_resilience, AppState};

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

const CODE247_SCOPE_INTENTIONS_WRITE: &str = "code247:intentions:write";
const CODE247_SCOPE_INTENTIONS_SYNC: &str = "code247:intentions:sync";
const CODE247_SERVICE_ROLE: &str = "service";
const CODE247_SERVICE_TOKEN_TTL_SECONDS: i64 = 300;

#[derive(Debug, Serialize)]
struct Code247ServiceClaims {
    sub: String,
    role: &'static str,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant_id: Option<String>,
    code247_projects: Vec<String>,
    iat: usize,
    exp: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
    app_metadata: Code247ServiceAppMetadata,
}

#[derive(Debug, Serialize)]
struct Code247ServiceAppMetadata {
    app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant_id: Option<String>,
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

fn build_code247_auth_token(
    state: &AppState,
    workspace: &str,
    project: &str,
    scopes: &[&str],
) -> Result<String> {
    if let Some(secret) = state
        .config
        .supabase_jwt_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let now = chrono::Utc::now().timestamp();
        let app_id = state
            .config
            .default_app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("edge-control")
            .to_string();
        let tenant_id = state
            .config
            .default_tenant_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let claims = Code247ServiceClaims {
            sub: app_id.clone(),
            role: CODE247_SERVICE_ROLE,
            scope: scopes.join(" "),
            tenant_id: tenant_id.clone(),
            code247_projects: vec![format!("{workspace}/{project}")],
            iat: now.max(0) as usize,
            exp: (now + CODE247_SERVICE_TOKEN_TTL_SECONDS).max(0) as usize,
            aud: state
                .config
                .supabase_jwt_audience
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            app_metadata: Code247ServiceAppMetadata { app_id, tenant_id },
        };
        return encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|err| anyhow!("failed to sign Code247 service JWT: {err}"));
    }

    if let Some(token) = state
        .config
        .code247_intentions_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(token.to_string());
    }

    Err(anyhow!(
        "Code247 auth not configured: define SUPABASE_JWT_SECRET or CODE247_INTENTIONS_TOKEN"
    ))
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
    let url_clone = url.clone();
    let payload_clone = payload.clone();
    let token = build_code247_auth_token(
        state,
        &request.workspace,
        &request.project,
        &[CODE247_SCOPE_INTENTIONS_WRITE],
    )?;
    let response = send_with_resilience(
        &state.config,
        &state.circuit_breakers,
        "code247.intentions",
        || {
            state
                .http_client
                .post(&url_clone)
                .json(&payload_clone)
                .bearer_auth(&token)
        },
    )
    .await?;
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
    let url_clone = url.clone();
    let payload_clone = payload.clone();
    let token =
        build_code247_auth_token(state, workspace, project, &[CODE247_SCOPE_INTENTIONS_SYNC])?;
    let response = send_with_resilience(
        &state.config,
        &state.circuit_breakers,
        "code247.intentions_sync",
        || {
            state
                .http_client
                .post(&url_clone)
                .json(&payload_clone)
                .bearer_auth(&token)
        },
    )
    .await?;
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

#[cfg(test)]
mod tests {
    use super::{
        build_code247_auth_token, CODE247_SCOPE_INTENTIONS_SYNC, CODE247_SCOPE_INTENTIONS_WRITE,
    };
    use crate::{
        config::{Config, IdempotencyBackend},
        policy::PolicySet,
        state_store::StateStore,
        AppState,
    };
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
    use serde_json::Value;
    use std::{env, path::PathBuf, sync::Arc};
    use uuid::Uuid;

    fn test_config() -> Config {
        let policy_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../policy/policy-set.v1.1.json");
        Config {
            host: "127.0.0.1".into(),
            port: 18080,
            policy_set_path: policy_path.display().to_string(),
            supabase_url: None,
            supabase_service_role_key: None,
            supabase_jwt_secret: Some("jwt-secret".into()),
            default_tenant_id: Some("voulezvous".into()),
            default_app_id: Some("edge-control".into()),
            default_user_id: Some("edge-control-system".into()),
            obs_api_base_url: None,
            obs_api_token: None,
            code247_base_url: "http://127.0.0.1:4001".into(),
            code247_intentions_token: Some("legacy-token".into()),
            supabase_jwks_url: None,
            supabase_jwt_audience: Some("authenticated".into()),
            internal_api_token: Some("internal-test-token".into()),
            rate_limit_window_seconds: 60,
            rate_limit_max_requests: 120,
            idempotency_ttl_seconds: 900,
            idempotency_backend: IdempotencyBackend::Sqlite,
            state_db_path: env::temp_dir()
                .join(format!(
                    "edge-control-orchestration-test-{}.db",
                    Uuid::new_v4()
                ))
                .display()
                .to_string(),
            resilience_max_retries: 1,
            resilience_initial_backoff_ms: 10,
            resilience_circuit_failures: 2,
            resilience_circuit_open_seconds: 5,
        }
    }

    fn test_state() -> Arc<AppState> {
        let config = test_config();
        let policy_set = PolicySet::load(&config.policy_set_path).expect("policy");
        let state_store = StateStore::from_config(&config).expect("state db");
        Arc::new(AppState::new(config, policy_set, state_store))
    }

    #[test]
    fn code247_auth_token_prefers_signed_jwt_with_project_scope() {
        let state = test_state();
        let token = build_code247_auth_token(
            &state,
            "voulezvous",
            "payments",
            &[
                CODE247_SCOPE_INTENTIONS_WRITE,
                CODE247_SCOPE_INTENTIONS_SYNC,
            ],
        )
        .expect("token");

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["authenticated"]);
        let claims = decode::<Value>(
            &token,
            &DecodingKey::from_secret("jwt-secret".as_bytes()),
            &validation,
        )
        .expect("decode")
        .claims;

        assert_eq!(claims["sub"], "edge-control");
        assert_eq!(claims["role"], "service");
        assert_eq!(claims["tenant_id"], "voulezvous");
        assert_eq!(
            claims["scope"],
            "code247:intentions:write code247:intentions:sync"
        );
        assert_eq!(claims["code247_projects"][0], "voulezvous/payments");
        assert_eq!(claims["app_metadata"]["app_id"], "edge-control");
    }

    #[test]
    fn code247_auth_token_falls_back_to_legacy_token() {
        let mut config = test_config();
        config.supabase_jwt_secret = None;
        let policy_set = PolicySet::load(&config.policy_set_path).expect("policy");
        let state_store = StateStore::from_config(&config).expect("state db");
        let state = Arc::new(AppState::new(config, policy_set, state_store));

        let token = build_code247_auth_token(
            &state,
            "voulezvous",
            "payments",
            &[CODE247_SCOPE_INTENTIONS_WRITE],
        )
        .expect("token");

        assert_eq!(token, "legacy-token");
    }

    #[test]
    fn code247_auth_token_requires_jwt_secret_or_legacy_token() {
        let mut config = test_config();
        config.supabase_jwt_secret = None;
        config.code247_intentions_token = None;
        let policy_set = PolicySet::load(&config.policy_set_path).expect("policy");
        let state_store = StateStore::from_config(&config).expect("state db");
        let state = Arc::new(AppState::new(config, policy_set, state_store));

        let err = build_code247_auth_token(
            &state,
            "voulezvous",
            "payments",
            &[CODE247_SCOPE_INTENTIONS_WRITE],
        )
        .expect_err("missing auth must fail");

        assert!(err.to_string().contains("Code247 auth not configured"));
    }
}
