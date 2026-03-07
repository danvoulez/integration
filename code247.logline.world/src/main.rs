mod adapters_rs;
mod api_rs;
mod branch_manager_rs;
mod config_rs;
mod context_builder_rs;
mod file_writer_rs;
mod manifest_validation_rs;
mod persistence_rs;
mod pipeline_rs;
mod policy_gate_rs;
mod pr_creator_rs;
mod resilience_rs;
mod risk_classifier_rs;
mod state_machine_rs;
mod supabase_sync_rs;
mod test_runner_rs;
mod transition_guard_rs;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, Result};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::Value;
use tokio::{signal, sync::Semaphore, task::JoinHandle, time};
use tracing::{error, info, warn};

use adapters_rs::{GitAdapter, LinearAdapter, LinearOAuthClient, LlmGatewayAdapter};
use branch_manager_rs::BranchManager;
use config_rs::Config;
use context_builder_rs::ContextBuilder;
use file_writer_rs::FileWriter;
use manifest_validation_rs::{validate_manifest, ManifestValidationConfig};
use persistence_rs::{
    CheckpointStore, EvidenceStore, ExecutionLogger, IntentionLinkRepository, JobsRepository,
    LinearOAuthTokenRepository, LinearOutboxAction, LinearOutboxRepository, LinearWebhookDelivery,
    LinearWebhookDeliveryRepository, ManifestIngestionRepository, OAuthStateRepository,
    RunTimelineRepository, SqliteDb,
};
use pipeline_rs::Pipeline;
use policy_gate_rs::PrRiskPolicy;
use pr_creator_rs::PrCreator;
use state_machine_rs::StateMachine;
use supabase_sync_rs::{spawn_sync_worker, SupabaseSyncConfig};
use test_runner_rs::TestRunner;
use transition_guard_rs::{
    classify_linear_workflow_state, is_linear_transition_allowed, LinearWorkflowState,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;
    if let Some(manifest_path) = validate_manifest(&ManifestValidationConfig {
        repo_root: PathBuf::from(&config.repo_root),
        manifest_path: PathBuf::from(&config.project_manifest_path),
        schema_path: PathBuf::from(&config.project_manifest_schema_path),
        required: config.project_manifest_required,
    })? {
        info!(manifest=%manifest_path.display(), "project manifest validated");
    } else {
        info!("project manifest not found; continuing without manifest enforcement");
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let (supabase_sync_handle, supabase_sync_worker) = if config.code247_supabase_sync_enabled {
        match (
            config.supabase_url.clone(),
            config.supabase_service_role_key.clone(),
            config.supabase_tenant_id.clone(),
            config.supabase_app_id.clone(),
        ) {
            (Some(url), Some(service_role_key), Some(tenant_id), Some(app_id)) => {
                let (handle, worker) = spawn_sync_worker(
                    SupabaseSyncConfig {
                        url,
                        service_role_key,
                        tenant_id,
                        app_id,
                        user_id: config.supabase_user_id.clone(),
                        realtime_enabled: config.code247_supabase_realtime_enabled,
                        realtime_channel: config.code247_supabase_realtime_channel.clone(),
                    },
                    shutdown_tx.subscribe(),
                );
                (Some(handle), Some(worker))
            }
            _ => {
                warn!(
                    "CODE247_SUPABASE_SYNC_ENABLED=true mas faltam envs obrigatórias (SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY, CODE247_SUPABASE_TENANT_ID, CODE247_SUPABASE_APP_ID); sync desativado"
                );
                (None, None)
            }
        }
    } else {
        info!("supabase sync disabled via CODE247_SUPABASE_SYNC_ENABLED=false");
        (None, None)
    };

    let db = SqliteDb::open(&config.db_path)?;
    db.run_migrations()?;

    let jobs = Arc::new(Mutex::new(JobsRepository::new(
        db.connection(),
        supabase_sync_handle.clone(),
    )));
    let checkpoints = Arc::new(Mutex::new(CheckpointStore::new(
        db.connection(),
        supabase_sync_handle.clone(),
    )));
    let evidence = Arc::new(EvidenceStore::new(config.evidence_path.clone()));
    let execution_logger = Arc::new(Mutex::new(ExecutionLogger::new(
        db.connection(),
        supabase_sync_handle,
    )));
    let oauth_states = Arc::new(Mutex::new(OAuthStateRepository::new(db.connection())));
    let oauth_tokens = Arc::new(Mutex::new(LinearOAuthTokenRepository::new(db.connection())));
    let manifest_ingestions = Arc::new(Mutex::new(ManifestIngestionRepository::new(
        db.connection(),
    )));
    let intention_links = Arc::new(Mutex::new(IntentionLinkRepository::new(db.connection())));
    let run_timeline = Arc::new(Mutex::new(RunTimelineRepository::new(db.connection())));
    let linear_outbox = Arc::new(Mutex::new(LinearOutboxRepository::new(db.connection())));
    let webhook_deliveries = Arc::new(Mutex::new(LinearWebhookDeliveryRepository::new(
        db.connection(),
    )));

    if config.linear_api_key.is_none() {
        info!("LINEAR_API_KEY ausente; fluxo legacy GraphQL via API key ficará indisponível");
    }
    let linear = LinearAdapter::new(
        config.linear_api_key.clone().unwrap_or_default(),
        config.linear_team_id.clone(),
        config.linear_api_base_url.clone(),
    );
    let git = GitAdapter::new(
        config.repo_root.clone(),
        config.git_branch.clone(),
        config.git_remote.clone(),
    );
    let llm = LlmGatewayAdapter::new(
        config.llm_gateway_url.clone(),
        config.llm_gateway_api_key.clone(),
    );

    let pr_creator = match (&config.github_token, &config.github_repo) {
        (Some(token), Some(repo)) => Some(PrCreator::new(
            token.clone(),
            repo.clone(),
            config.git_branch.clone(),
            config.github_auto_merge_enabled,
            config.github_auto_merge_timeout_seconds,
            config.github_auto_merge_poll_seconds,
        )),
        _ => None,
    };
    let pr_policy =
        PrRiskPolicy::load_from_path(&config.policy_set_path, config.policy_set_required)?;
    let pr_policy_meta = pr_policy.metadata();
    info!(
        policy_version = %pr_policy_meta.version,
        policy_path = %pr_policy_meta.source_path,
        policy_sha256 = %pr_policy_meta.source_sha256,
        "pr-risk policy loaded"
    );

    let pipeline = Arc::new(Pipeline::new(
        jobs.clone(),
        checkpoints,
        evidence,
        execution_logger.clone(),
        StateMachine::default(),
        llm,
        git.clone(),
        linear.clone(),
        BranchManager::new(git.clone()),
        FileWriter::new(config.repo_root.clone()),
        ContextBuilder::new(config.voulezvous_spec_path.clone(), linear.clone()),
        TestRunner::new(
            config.repo_root.clone(),
            config.ci_flaky_reruns,
            config.red_main_enforced,
            config.red_main_flag_path.clone(),
            config.code247_runner_allowlist_enabled,
            config.code247_runner_allowlist_manifest_path.clone(),
        ),
        pr_policy,
        linear_outbox.clone(),
        pr_creator,
        config.max_review_iterations,
        config.stage_lease_owner.clone(),
        config.stage_timeout_planning_seconds,
        config.stage_timeout_coding_seconds,
        config.stage_timeout_reviewing_seconds,
        config.stage_timeout_validating_seconds,
        config.stage_timeout_committing_seconds,
        config.linear_claim_in_progress_state_name.clone(),
        config.linear_ready_for_release_state_name.clone(),
        config.linear_done_state_type.clone(),
    ));

    let worker = spawn_worker(
        jobs.clone(),
        pipeline,
        linear.clone(),
        config.poll_interval_ms,
        config.max_concurrent_jobs,
        config.stage_lease_owner.clone(),
        config.stage_timeout_planning_seconds,
        shutdown_rx,
    );
    let oauth_client = if config.linear_oauth_enabled() {
        Some(LinearOAuthClient::new(
            config.linear_client_id.clone().expect("client_id checked"),
            config
                .linear_client_secret
                .clone()
                .expect("client_secret checked"),
            config
                .linear_oauth_redirect_uri
                .clone()
                .expect("redirect_uri checked"),
            config.linear_oauth_scopes.clone(),
            config.linear_oauth_actor.clone(),
            config.linear_oauth_base_url.clone(),
        ))
    } else {
        None
    };
    let oauth_refresh_worker = oauth_client.as_ref().map(|client| {
        spawn_oauth_refresh_worker(
            oauth_tokens.clone(),
            client.clone(),
            config.linear_oauth_refresh_interval_seconds,
            config.linear_oauth_refresh_lead_seconds,
            shutdown_tx.subscribe(),
        )
    });
    let linear_claim_worker = if config.linear_claim_enabled {
        Some(spawn_linear_claim_worker(
            jobs.clone(),
            linear.clone(),
            config.linear_claim_state_name.clone(),
            config.linear_claim_in_progress_state_name.clone(),
            config.linear_ready_for_release_state_name.clone(),
            config.linear_done_state_type.clone(),
            config.linear_claim_interval_seconds,
            config.linear_claim_max_per_cycle,
            shutdown_tx.subscribe(),
        ))
    } else {
        info!("linear auto-claim worker disabled via LINEAR_CLAIM_ENABLED=false");
        None
    };
    let webhook_worker = if config
        .linear_webhook_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        Some(spawn_linear_webhook_worker(
            webhook_deliveries.clone(),
            jobs.clone(),
            config.linear_claim_state_name.clone(),
            config.linear_claim_in_progress_state_name.clone(),
            config.linear_ready_for_release_state_name.clone(),
            config.linear_done_state_type.clone(),
            config.linear_webhook_poll_interval_seconds,
            config.linear_webhook_retry_delay_seconds,
            config.linear_webhook_max_attempts,
            shutdown_tx.subscribe(),
        ))
    } else {
        info!("linear webhook worker disabled (LINEAR_WEBHOOK_SECRET not configured)");
        None
    };
    let linear_outbox_worker = Some(spawn_linear_outbox_worker(
        linear_outbox.clone(),
        linear.clone(),
        config.linear_webhook_poll_interval_seconds,
        config.linear_webhook_retry_delay_seconds,
        config.linear_webhook_max_attempts,
        shutdown_tx.subscribe(),
    ));
    let stage_lease_sweeper = Some(spawn_stage_lease_sweeper(
        jobs.clone(),
        execution_logger.clone(),
        linear_outbox.clone(),
        config.stage_lease_sweep_interval_seconds,
        shutdown_tx.subscribe(),
    ));
    let api = tokio::spawn(api_rs::serve(
        config.clone(),
        jobs,
        run_timeline,
        oauth_states,
        oauth_tokens,
        manifest_ingestions,
        intention_links,
        webhook_deliveries,
        oauth_client,
    ));

    signal::ctrl_c().await?;
    info!("shutdown signal received");
    let _ = shutdown_tx.send(true);

    worker.await??;
    if let Some(claim_worker) = linear_claim_worker {
        claim_worker.await??;
    }
    if let Some(webhook_worker) = webhook_worker {
        webhook_worker.await??;
    }
    if let Some(linear_outbox_worker) = linear_outbox_worker {
        linear_outbox_worker.await??;
    }
    if let Some(stage_lease_sweeper) = stage_lease_sweeper {
        stage_lease_sweeper.await??;
    }
    if let Some(refresh_worker) = oauth_refresh_worker {
        refresh_worker.await??;
    }
    if let Some(sync_worker) = supabase_sync_worker {
        sync_worker.await??;
    }
    api.abort();
    Ok(())
}

fn spawn_worker(
    jobs: Arc<Mutex<JobsRepository>>,
    pipeline: Arc<Pipeline>,
    linear: LinearAdapter,
    poll_interval_ms: u64,
    max_concurrent_jobs: usize,
    stage_lease_owner: String,
    planning_timeout_seconds: i64,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(max_concurrent_jobs));
        let mut ticker = time::interval(Duration::from_millis(poll_interval_ms));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let maybe_job = jobs
                        .lock()
                        .expect("lock jobs")
                        .claim_next_pending_with_lease(&stage_lease_owner, planning_timeout_seconds);
                    if let Some(job) = maybe_job {
                        let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
                        let jobs_ref = jobs.clone();
                        let pipeline_ref = pipeline.clone();
                        let linear_ref = linear.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(err) = pipeline_ref.run(job.clone()).await {
                                error!(job_id=%job.id, error=%err, "job failed");
                                {
                                    let mut repo = jobs_ref.lock().expect("lock jobs");
                                    repo.increment_retries(&job.id);
                                    repo.update_status(
                                        &job.id,
                                        persistence_rs::JobStatus::Failed,
                                        Some(err.to_string()),
                                    );
                                }
                                let comment = format!(
                                    "`code247:failed` run_id=`{}` reason=`{}`",
                                    job.id,
                                    err.to_string().replace('`', "'")
                                );
                                if let Err(comment_err) =
                                    linear_ref.create_comment(&job.issue_id, &comment).await
                                {
                                    warn!(
                                        issue_id=%job.issue_id,
                                        error=%comment_err,
                                        "failed to post failure comment to Linear"
                                    );
                                }
                            } else {
                                info!(job_id=%job.id, "job completed");
                            }
                        });
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("worker shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}

fn spawn_linear_claim_worker(
    jobs: Arc<Mutex<JobsRepository>>,
    linear: LinearAdapter,
    claim_state_name: String,
    claim_in_progress_state_name: String,
    ready_for_release_state_name: String,
    done_state_type: String,
    claim_interval_seconds: u64,
    claim_max_per_cycle: usize,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(claim_interval_seconds.max(5)));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let issues = match linear.list_team_issues(Some(&claim_state_name)).await {
                        Ok(items) => items,
                        Err(err) => {
                            error!(error=%err, state_name=%claim_state_name, "linear claim list failed");
                            continue;
                        }
                    };
                    let in_progress_state_id = match linear
                        .find_state_id_by_name(&claim_in_progress_state_name)
                        .await
                    {
                        Ok(state_id) => Some(state_id),
                        Err(err) => {
                            error!(
                                error=%err,
                                state_name=%claim_in_progress_state_name,
                                "failed to resolve in-progress state; proceeding without Linear state transition"
                            );
                            None
                        }
                    };

                    let mut claimed = 0usize;
                    for issue in issues {
                        if claimed >= claim_max_per_cycle {
                            break;
                        }
                        let current_state = classify_linear_workflow_state(
                            &issue.state.name,
                            &issue.state.r#type,
                            &claim_state_name,
                            &claim_in_progress_state_name,
                            &ready_for_release_state_name,
                            &done_state_type,
                        );
                        if current_state == LinearWorkflowState::Done {
                            continue;
                        }
                        if !is_linear_transition_allowed(
                            current_state,
                            LinearWorkflowState::InProgress,
                        ) {
                            continue;
                        }

                        let payload = issue
                            .description
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .unwrap_or(issue.title.as_str())
                            .to_string();

                        let created_job = {
                            let mut repo = jobs.lock().expect("lock jobs");
                            if repo.has_non_failed_job_for_issue(&issue.id) {
                                None
                            } else {
                                Some(repo.create_job(&issue.id, &payload))
                            }
                        };

                        let Some(created_job) = created_job else {
                            continue;
                        };

                        match created_job {
                            Ok(job) => {
                                claimed += 1;
                                if let Some(state_id) = in_progress_state_id.as_deref() {
                                    if let Err(err) = linear.update_issue_state(&issue.id, state_id).await {
                                        error!(
                                            issue_id=%issue.id,
                                            issue_identifier=%issue.identifier,
                                            target_state=%claim_in_progress_state_name,
                                            error=%err,
                                            "failed to move claimed issue to in-progress state"
                                        );
                                    }
                                }
                                info!(
                                    job_id=%job.id,
                                    issue_id=%issue.id,
                                    issue_identifier=%issue.identifier,
                                    "linear issue auto-claimed into pending job"
                                );
                            }
                            Err(err) => {
                                error!(
                                    issue_id=%issue.id,
                                    issue_identifier=%issue.identifier,
                                    error=%err,
                                    "failed to auto-claim linear issue"
                                );
                            }
                        }
                    }

                    if claimed > 0 {
                        info!(claimed, state_name=%claim_state_name, "linear claim cycle completed");
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("linear claim worker shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}

fn spawn_linear_webhook_worker(
    webhook_store: Arc<Mutex<LinearWebhookDeliveryRepository>>,
    jobs: Arc<Mutex<JobsRepository>>,
    claim_state_name: String,
    claim_in_progress_state_name: String,
    ready_for_release_state_name: String,
    done_state_type: String,
    poll_interval_seconds: u64,
    retry_delay_seconds: u64,
    max_attempts: i32,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(poll_interval_seconds.max(2)));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let maybe_delivery = {
                        let store = webhook_store.lock().expect("lock webhook store");
                        store.claim_next_ready()
                    };
                    let Some(delivery) = maybe_delivery else {
                        continue;
                    };

                    let result = process_linear_webhook_delivery(
                        &delivery,
                        jobs.clone(),
                        &claim_state_name,
                        &claim_in_progress_state_name,
                        &ready_for_release_state_name,
                        &done_state_type,
                    );
                    match result {
                        Ok(outcome) => {
                            {
                                let store = webhook_store.lock().expect("lock webhook store");
                                if let Err(err) = store.mark_done(&delivery.delivery_id) {
                                    error!(
                                        delivery_id=%delivery.delivery_id,
                                        error=%err,
                                        "failed to mark webhook delivery done"
                                    );
                                }
                            }
                            info!(
                                delivery_id=%delivery.delivery_id,
                                issue_id=?delivery.issue_id,
                                event=?delivery.linear_event,
                                outcome=%outcome,
                                "linear webhook delivery processed"
                            );
                        }
                        Err(err) => {
                            {
                                let store = webhook_store.lock().expect("lock webhook store");
                                if let Err(mark_err) = store.mark_retry_or_dlq(
                                    &delivery.delivery_id,
                                    delivery.attempts,
                                    max_attempts.max(1),
                                    retry_delay_seconds as i64,
                                    &err.to_string(),
                                ) {
                                    error!(
                                        delivery_id=%delivery.delivery_id,
                                        error=%mark_err,
                                        "failed to update webhook delivery retry/dlq state"
                                    );
                                }
                            }
                            warn!(
                                delivery_id=%delivery.delivery_id,
                                attempts=delivery.attempts,
                                max_attempts=max_attempts,
                                error=%err,
                                "linear webhook delivery processing failed"
                            );
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("linear webhook worker shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}

fn process_linear_webhook_delivery(
    delivery: &LinearWebhookDelivery,
    jobs: Arc<Mutex<JobsRepository>>,
    claim_state_name: &str,
    claim_in_progress_state_name: &str,
    ready_for_release_state_name: &str,
    done_state_type: &str,
) -> Result<String> {
    let payload: Value = serde_json::from_str(&delivery.payload)
        .map_err(|err| anyhow!("invalid webhook payload JSON: {err}"))?;
    let event = delivery
        .linear_event
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    if !event.eq_ignore_ascii_case("Issue") {
        return Ok(format!("ignored non-Issue event: {event}"));
    }

    let action = payload
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    if action.eq_ignore_ascii_case("remove") {
        return Ok("ignored remove action".to_string());
    }

    let Some(issue_id) = payload
        .pointer("/data/id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .pointer("/data/issue/id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
    else {
        return Ok("ignored payload without issue id".to_string());
    };

    let state_name = payload
        .pointer("/data/state/name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let state_type = payload
        .pointer("/data/state/type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let current_state = classify_linear_workflow_state(
        state_name,
        state_type,
        claim_state_name,
        claim_in_progress_state_name,
        ready_for_release_state_name,
        done_state_type,
    );
    if current_state == LinearWorkflowState::Done {
        return Ok(format!("ignored completed issue {issue_id}"));
    }
    let should_claim_from_state = current_state == LinearWorkflowState::Ready
        && is_linear_transition_allowed(current_state, LinearWorkflowState::InProgress);
    let should_claim_from_label = extract_queue_labels(&payload)
        .iter()
        .any(|label| label.eq_ignore_ascii_case("code247:queue"));
    if !should_claim_from_state && !should_claim_from_label {
        return Ok(format!(
            "ignored issue {issue_id}: state='{state_name}', queue_label={should_claim_from_label}"
        ));
    }

    let payload_text = payload
        .pointer("/data/description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            payload
                .pointer("/data/title")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| format!("Linear webhook {action} for issue {issue_id}"));

    let mut jobs_repo = jobs.lock().expect("lock jobs");
    if jobs_repo.has_non_failed_job_for_issue(&issue_id) {
        return Ok(format!("deduped existing active job for issue {issue_id}"));
    }
    let job = jobs_repo.create_job(&issue_id, &payload_text)?;
    Ok(format!("created job {} for issue {}", job.id, issue_id))
}

fn spawn_linear_outbox_worker(
    outbox: Arc<Mutex<LinearOutboxRepository>>,
    linear: LinearAdapter,
    poll_interval_seconds: u64,
    retry_delay_seconds: u64,
    max_attempts: i32,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(poll_interval_seconds.max(2)));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let maybe_action = {
                        let store = outbox.lock().expect("lock linear outbox");
                        store.claim_next_ready()
                    };
                    let Some(action) = maybe_action else {
                        continue;
                    };

                    match process_linear_outbox_action(&linear, &action).await {
                        Ok(()) => {
                            let store = outbox.lock().expect("lock linear outbox");
                            if let Err(err) = store.mark_done(&action.id) {
                                error!(action_id=%action.id, error=%err, "failed to mark linear outbox action done");
                            }
                        }
                        Err(err) => {
                            let store = outbox.lock().expect("lock linear outbox");
                            if let Err(mark_err) = store.mark_retry_or_dlq(
                                &action.id,
                                action.attempts,
                                max_attempts.max(1),
                                retry_delay_seconds as i64,
                                &err.to_string(),
                            ) {
                                error!(action_id=%action.id, error=%mark_err, "failed to update linear outbox retry state");
                            }
                            warn!(
                                action_id=%action.id,
                                issue_id=%action.issue_id,
                                action_type=%action.action_type,
                                attempts=action.attempts,
                                error=%err,
                                "linear outbox action failed"
                            );
                        }
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("linear outbox worker shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}

fn spawn_stage_lease_sweeper(
    jobs: Arc<Mutex<JobsRepository>>,
    execution_logger: Arc<Mutex<ExecutionLogger>>,
    linear_outbox: Arc<Mutex<LinearOutboxRepository>>,
    interval_seconds: u64,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(interval_seconds.max(5)));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let expired = {
                        let repo = jobs.lock().expect("lock jobs");
                        repo.list_expired_stage_leases(&Utc::now())
                    };

                    for lease in expired {
                        let lease_expires_at = lease.lease_expires_at.clone().unwrap_or_else(|| "unknown".to_string());
                        let error_message = format!(
                            "stage lease expired: status={} lease_expires_at={} owner={}",
                            lease.status.as_str(),
                            lease_expires_at,
                            lease.lease_owner.clone().unwrap_or_else(|| "unknown".to_string())
                        );
                        let changed = {
                            let mut repo = jobs.lock().expect("lock jobs");
                            repo.expire_stage_lease(
                                &lease.id,
                                lease.status,
                                lease.lease_owner.as_deref(),
                                lease.lease_expires_at.as_deref(),
                                &error_message,
                            )
                        };
                        if !changed {
                            continue;
                        }

                        execution_logger
                            .lock()
                            .expect("logger lock")
                            .log_stage(
                                &lease.id,
                                "lease_expired",
                                &serde_json::to_string(&serde_json::json!({
                                    "status": lease.status.as_str(),
                                    "stage_started_at": lease.stage_started_at,
                                    "heartbeat_at": lease.heartbeat_at,
                                    "lease_expires_at": lease.lease_expires_at,
                                    "lease_owner": lease.lease_owner,
                                    "stage_attempt": lease.stage_attempt,
                                }))?,
                                &serde_json::to_string(&serde_json::json!({
                                    "error": error_message,
                                    "action": "failed",
                                    "mode": "stage_lease_enforcement",
                                }))?,
                                "stage-lease:v1",
                                0,
                            );

                        if !lease.issue_id.trim().is_empty()
                            && !lease.issue_id.starts_with("smoke:")
                        {
                            if let Err(err) = linear_outbox
                                .lock()
                                .expect("linear outbox lock")
                                .enqueue(
                                    &lease.issue_id,
                                    "comment",
                                    &serde_json::json!({
                                        "body": format!(
                                            "`code247:lease-expired` run_id=`{}` stage=`{}` lease_expires_at=`{}` action=`failed`",
                                            lease.id,
                                            lease.status.as_str(),
                                            lease_expires_at
                                        )
                                    }),
                                )
                            {
                                error!(job_id=%lease.id, issue_id=%lease.issue_id, error=%err, "failed to enqueue stage lease expiration comment");
                            }
                        }

                        warn!(
                            job_id=%lease.id,
                            issue_id=%lease.issue_id,
                            status=%lease.status.as_str(),
                            lease_expires_at=%lease_expires_at,
                            "stage lease expired and job auto-escalated to failed"
                        );
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("stage lease sweeper shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}

async fn process_linear_outbox_action(
    linear: &LinearAdapter,
    action: &LinearOutboxAction,
) -> Result<()> {
    let payload: Value = serde_json::from_str(&action.payload)
        .map_err(|err| anyhow!("invalid outbox payload JSON: {err}"))?;

    match action.action_type.as_str() {
        "comment" => {
            let body = payload
                .get("body")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("missing outbox comment body"))?;
            linear.create_comment(&action.issue_id, body).await
        }
        "transition" => {
            let state_name = payload
                .get("state_name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("missing outbox state_name"))?;
            let state_id = linear.find_state_id_by_name(state_name).await?;
            linear.update_issue_state(&action.issue_id, &state_id).await
        }
        other => Err(anyhow!("unsupported linear outbox action: {other}")),
    }
}

fn extract_queue_labels(payload: &Value) -> Vec<String> {
    if let Some(nodes) = payload
        .pointer("/data/labels/nodes")
        .and_then(Value::as_array)
    {
        return nodes
            .iter()
            .filter_map(|node| node.get("name").and_then(Value::as_str))
            .map(ToString::to_string)
            .collect();
    }
    if let Some(labels) = payload.pointer("/data/labels").and_then(Value::as_array) {
        return labels
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect();
    }
    Vec::new()
}

fn spawn_oauth_refresh_worker(
    token_store: Arc<Mutex<LinearOAuthTokenRepository>>,
    oauth_client: LinearOAuthClient,
    refresh_interval_seconds: u64,
    refresh_lead_seconds: i64,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(refresh_interval_seconds.max(15)));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let maybe_due_token = {
                        let store = token_store.lock().expect("lock oauth token store");
                        store.token_due_for_refresh(refresh_lead_seconds)
                    };

                    let Some(current_token) = maybe_due_token else {
                        continue;
                    };

                    let refreshed = match oauth_client.refresh_token(&current_token.refresh_token).await {
                        Ok(value) => value,
                        Err(err) => {
                            error!(error=%err, "linear oauth refresh failed");
                            continue;
                        }
                    };

                    let refresh_token = refreshed
                        .refresh_token
                        .unwrap_or(current_token.refresh_token);
                    let expires_at =
                        (Utc::now() + ChronoDuration::seconds(refreshed.expires_in.max(60)))
                            .to_rfc3339();

                    let upsert_result = {
                        let store = token_store.lock().expect("lock oauth token store");
                        store.upsert_token(
                            &refreshed.access_token,
                            &refresh_token,
                            &refreshed.token_type,
                            refreshed.scope.as_deref(),
                            &expires_at,
                        )
                    };
                    if let Err(err) = upsert_result {
                        error!(error=%err, "linear oauth refresh persistence failed");
                        continue;
                    }

                    info!(expires_at=%expires_at, "linear oauth token refreshed");
                }
                changed = shutdown.changed() => {
                    if changed.is_ok() && *shutdown.borrow() {
                        info!("oauth refresh worker shutdown complete");
                        return Ok(());
                    }
                }
            }
        }
    })
}
