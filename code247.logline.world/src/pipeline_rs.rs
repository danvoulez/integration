use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::warn;

use crate::{
    adapters_rs::{CloudGateDecision, GitAdapter, LinearAdapter, LlmGatewayAdapter, ReviewOutput},
    branch_manager_rs::BranchManager,
    context_builder_rs::ContextBuilder,
    file_writer_rs::FileWriter,
    persistence_rs::{
        CheckpointStore, EvidenceStore, ExecutionLogger, Job, JobStatus, JobsRepository,
        LinearOutboxRepository,
    },
    policy_gate_rs::{PlanGovernancePolicy, PolicyEvaluation, PrRiskPolicy},
    pr_creator_rs::PrCreator,
    risk_classifier_rs::{MergeMode, RiskClassifier},
    state_machine_rs::StateMachine,
    test_runner_rs::TestRunner,
    transition_guard_rs::{
        classify_linear_workflow_state, is_linear_transition_allowed, LinearWorkflowState,
    },
};

pub struct Pipeline {
    jobs: Arc<Mutex<JobsRepository>>,
    checkpoints: Arc<Mutex<CheckpointStore>>,
    evidence: Arc<EvidenceStore>,
    execution_logger: Arc<Mutex<ExecutionLogger>>,
    fsm: StateMachine,
    llm: LlmGatewayAdapter,
    git: GitAdapter,
    linear: LinearAdapter,
    branch_manager: BranchManager,
    file_writer: FileWriter,
    context_builder: ContextBuilder,
    test_runner: TestRunner,
    pr_policy: PrRiskPolicy,
    linear_outbox: Arc<Mutex<LinearOutboxRepository>>,
    pr_creator: Option<PrCreator>,
    max_review_iterations: u8,
    lease_owner: String,
    planning_timeout_seconds: i64,
    coding_timeout_seconds: i64,
    reviewing_timeout_seconds: i64,
    validating_timeout_seconds: i64,
    committing_timeout_seconds: i64,
    linear_in_progress_state_name: String,
    linear_ready_for_release_state_name: String,
    linear_done_state_type: String,
}

#[derive(Debug, Clone)]
struct PlanGovernanceEvidence {
    objective_present: bool,
    changes_present: bool,
    risk_present: bool,
    acceptance: Vec<String>,
    how_to_test: Vec<String>,
    backout: Vec<String>,
}

impl PlanGovernanceEvidence {
    fn to_markdown_appendix(&self, checks_url: &str) -> String {
        format!(
            "## Plan Contract\n- objective_present: `{}`\n- changes_present: `{}`\n- risk_present: `{}`\n\n## Acceptance Criteria\n{}\n\n## How To Test\n{}\n\n## Backout / Rollback\n{}\n\n## Checks\n- {checks_url}",
            self.objective_present,
            self.changes_present,
            self.risk_present,
            markdown_bullets(&self.acceptance),
            markdown_bullets(&self.how_to_test),
            markdown_bullets(&self.backout),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MergePolicyOverrideAction {
    AllowAutoMerge,
    ForceManualReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MergePolicyOverride {
    action: MergePolicyOverrideAction,
    actor: String,
    reason: String,
    ticket: Option<String>,
    source: Option<String>,
    approved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MergePolicyResolution {
    AutoMerge,
    ManualReview,
}

#[derive(Debug, Clone, Serialize)]
struct MergePolicyDecision {
    merge_mode: String,
    risk_score: u8,
    auto_merge_allowed: bool,
    resolution: MergePolicyResolution,
    reason: String,
    override_applied: Option<MergePolicyOverride>,
    cloud_gate: Option<CloudGateDecision>,
    cloud_policy: Option<PolicyEvaluation>,
}

fn markdown_bullets(items: &[String]) -> String {
    if items.is_empty() {
        "- (none)".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn validate_plan_governance(
    plan: &str,
    policy: &PlanGovernancePolicy,
) -> Result<PlanGovernanceEvidence> {
    let normalized = plan.to_ascii_lowercase();
    let objective_present = contains_any(&normalized, &["objetivo", "goal"]);
    let changes_present = contains_any(&normalized, &["mudanças", "mudancas", "changes"]);
    let risk_present = contains_any(&normalized, &["risco", "risk"]);
    let acceptance = extract_section_items(
        plan,
        &[
            "acceptance criteria",
            "acceptance",
            "aceitação",
            "aceitacao",
        ],
    );
    let how_to_test = extract_section_items(
        plan,
        &[
            "how-to-test",
            "how to test",
            "como testar",
            "validação",
            "validacao",
            "validation",
        ],
    );
    let backout = extract_section_items(plan, &["backout", "rollback"]);

    let mut missing = Vec::new();
    if policy.require_objective && !objective_present {
        missing.push("objetivo/goal");
    }
    if policy.require_changes && !changes_present {
        missing.push("mudanças/changes");
    }
    if policy.require_risk && !risk_present {
        missing.push("risco/risk");
    }
    if policy.require_acceptance && acceptance.is_empty() {
        missing.push("acceptance criteria");
    }
    if policy.require_how_to_test && how_to_test.is_empty() {
        missing.push("how-to-test");
    }
    if policy.require_backout && backout.is_empty() {
        missing.push("backout/rollback");
    }

    if !missing.is_empty() {
        bail!(
            "plano fora do contrato obrigatório; faltando seção/campo: {}",
            missing.join(", ")
        );
    }

    Ok(PlanGovernanceEvidence {
        objective_present,
        changes_present,
        risk_present,
        acceptance,
        how_to_test,
        backout,
    })
}

fn parse_merge_policy_override(payload: &str) -> Option<MergePolicyOverride> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    let raw = value
        .pointer("/merge_policy/override")
        .or_else(|| value.pointer("/controls/merge_policy/override"))?
        .clone();
    let mut parsed = serde_json::from_value::<MergePolicyOverride>(raw).ok()?;
    parsed.actor = parsed.actor.trim().to_string();
    parsed.reason = parsed.reason.trim().to_string();
    parsed.ticket = parsed
        .ticket
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    parsed.source = parsed
        .source
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    parsed.approved_at = parsed
        .approved_at
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if parsed.actor.is_empty() || parsed.reason.is_empty() {
        return None;
    }
    Some(parsed)
}

fn resolve_merge_policy_decision(
    payload: &str,
    risk: &crate::risk_classifier_rs::RiskAssessment,
    cloud_gate: Option<&CloudGateDecision>,
    policy: &PrRiskPolicy,
) -> MergePolicyDecision {
    let override_request = parse_merge_policy_override(payload);
    let cloud_policy = cloud_gate.map(|decision| policy.evaluate_cloud_decision(decision));
    let merge_mode = match risk.merge_mode {
        MergeMode::Light => "light",
        MergeMode::Substantial => "substantial",
    }
    .to_string();

    if let Some(override_request) = override_request.clone() {
        match override_request.action {
            MergePolicyOverrideAction::ForceManualReview => {
                return MergePolicyDecision {
                    merge_mode,
                    risk_score: risk.score,
                    auto_merge_allowed: false,
                    resolution: MergePolicyResolution::ManualReview,
                    reason: format!(
                        "manual review forced by override from '{}' ({})",
                        override_request.actor, override_request.reason
                    ),
                    override_applied: Some(override_request),
                    cloud_gate: cloud_gate.cloned(),
                    cloud_policy,
                };
            }
            MergePolicyOverrideAction::AllowAutoMerge => {
                if risk.merge_mode == MergeMode::Substantial
                    && cloud_policy.as_ref().is_some_and(|eval| !eval.allowed)
                {
                    return MergePolicyDecision {
                        merge_mode,
                        risk_score: risk.score,
                        auto_merge_allowed: true,
                        resolution: MergePolicyResolution::AutoMerge,
                        reason: format!(
                            "substantial auto-merge override approved by '{}' ({})",
                            override_request.actor, override_request.reason
                        ),
                        override_applied: Some(override_request),
                        cloud_gate: cloud_gate.cloned(),
                        cloud_policy,
                    };
                }
            }
        }
    }

    match risk.merge_mode {
        MergeMode::Light => MergePolicyDecision {
            merge_mode,
            risk_score: risk.score,
            auto_merge_allowed: true,
            resolution: MergePolicyResolution::AutoMerge,
            reason: "light merge eligible by default policy".to_string(),
            override_applied: None,
            cloud_gate: None,
            cloud_policy: None,
        },
        MergeMode::Substantial => {
            if let Some(policy_eval) = cloud_policy.clone() {
                if policy_eval.allowed {
                    MergePolicyDecision {
                        merge_mode,
                        risk_score: risk.score,
                        auto_merge_allowed: true,
                        resolution: MergePolicyResolution::AutoMerge,
                        reason: "substantial merge approved by cloud gate policy".to_string(),
                        override_applied: None,
                        cloud_gate: cloud_gate.cloned(),
                        cloud_policy: Some(policy_eval),
                    }
                } else {
                    MergePolicyDecision {
                        merge_mode,
                        risk_score: risk.score,
                        auto_merge_allowed: false,
                        resolution: MergePolicyResolution::ManualReview,
                        reason: format!(
                            "substantial merge blocked by cloud gate policy: {}",
                            policy_eval.reason
                        ),
                        override_applied: None,
                        cloud_gate: cloud_gate.cloned(),
                        cloud_policy: Some(policy_eval),
                    }
                }
            } else {
                MergePolicyDecision {
                    merge_mode,
                    risk_score: risk.score,
                    auto_merge_allowed: false,
                    resolution: MergePolicyResolution::ManualReview,
                    reason: "substantial merge requires cloud gate decision".to_string(),
                    override_applied: None,
                    cloud_gate: None,
                    cloud_policy: None,
                }
            }
        }
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn extract_section_items(plan: &str, section_keywords: &[&str]) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;

    for line in plan.lines() {
        let trimmed = line.trim();
        let normalized = trimmed.to_ascii_lowercase();

        if contains_any(&normalized, section_keywords) {
            in_section = true;
            let inline = inline_value_after_colon(trimmed);
            if let Some(value) = inline {
                items.push(value);
            }
            continue;
        }

        if !in_section {
            continue;
        }

        if trimmed.is_empty() && !items.is_empty() {
            break;
        }
        if is_heading_line(&normalized) {
            break;
        }

        let cleaned = strip_list_prefix(trimmed);
        if !cleaned.is_empty() {
            items.push(cleaned.to_string());
        }
    }

    items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn inline_value_after_colon(line: &str) -> Option<String> {
    let value = line.split_once(':')?.1.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn is_heading_line(line_lower: &str) -> bool {
    line_lower.starts_with('#')
        || line_lower.ends_with(':')
        || line_lower.starts_with("objetivo")
        || line_lower.starts_with("goal")
        || line_lower.starts_with("mudanças")
        || line_lower.starts_with("mudancas")
        || line_lower.starts_with("changes")
        || line_lower.starts_with("acceptance")
        || line_lower.starts_with("aceitação")
        || line_lower.starts_with("aceitacao")
        || line_lower.starts_with("how-to-test")
        || line_lower.starts_with("how to test")
        || line_lower.starts_with("como testar")
        || line_lower.starts_with("risco")
        || line_lower.starts_with("risk")
        || line_lower.starts_with("backout")
        || line_lower.starts_with("rollback")
}

fn strip_list_prefix(input: &str) -> &str {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        return rest.trim();
    }
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return rest.trim();
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return rest.trim();
    }
    if let Some((prefix, rest)) = trimmed.split_once('.') {
        if !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_digit()) {
            return rest.trim();
        }
    }
    trimmed
}

impl Pipeline {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        jobs: Arc<Mutex<JobsRepository>>,
        checkpoints: Arc<Mutex<CheckpointStore>>,
        evidence: Arc<EvidenceStore>,
        execution_logger: Arc<Mutex<ExecutionLogger>>,
        fsm: StateMachine,
        llm: LlmGatewayAdapter,
        git: GitAdapter,
        linear: LinearAdapter,
        branch_manager: BranchManager,
        file_writer: FileWriter,
        context_builder: ContextBuilder,
        test_runner: TestRunner,
        pr_policy: PrRiskPolicy,
        linear_outbox: Arc<Mutex<LinearOutboxRepository>>,
        pr_creator: Option<PrCreator>,
        max_review_iterations: u8,
        lease_owner: String,
        planning_timeout_seconds: i64,
        coding_timeout_seconds: i64,
        reviewing_timeout_seconds: i64,
        validating_timeout_seconds: i64,
        committing_timeout_seconds: i64,
        linear_in_progress_state_name: String,
        linear_ready_for_release_state_name: String,
        linear_done_state_type: String,
    ) -> Self {
        Self {
            jobs,
            checkpoints,
            evidence,
            execution_logger,
            fsm,
            llm,
            git,
            linear,
            branch_manager,
            file_writer,
            context_builder,
            test_runner,
            pr_policy,
            linear_outbox,
            pr_creator,
            max_review_iterations,
            lease_owner,
            planning_timeout_seconds,
            coding_timeout_seconds,
            reviewing_timeout_seconds,
            validating_timeout_seconds,
            committing_timeout_seconds,
            linear_in_progress_state_name,
            linear_ready_for_release_state_name,
            linear_done_state_type,
        }
    }

    pub async fn run(&self, mut job: Job) -> Result<()> {
        let issue = self.linear.get_issue(&job.issue_id).await?;
        let backlog = self.linear.list_team_issues(None).await?;
        let tracked = backlog.iter().any(|i| i.id == issue.id);
        if !tracked {
            return Err(anyhow!(
                "issue {} não pertence ao backlog do time configurado",
                issue.id
            ));
        }
        if issue.state.r#type.eq_ignore_ascii_case("completed") {
            self.transition(&mut job, JobStatus::Done)?;
            return Ok(());
        }

        self.transition(&mut job, JobStatus::Planning)?;

        let planning_prompt = self
            .context_builder
            .build_planning_prompt(&job.issue_id, &job.payload)
            .await?;

        self.branch_manager.ensure_clean().await?;
        let branch = self
            .branch_manager
            .create_job_branch(&issue.identifier)
            .await?;
        self.queue_linear_comment(
            &issue.id,
            format!(
                "`code247:running` run_id=`{}` branch=`{}` status=`planning`",
                job.id, branch
            ),
        )
        .await;
        self.queue_linear_state_transition(&issue.id, &self.linear_in_progress_state_name)
            .await?;
        let plan = if let Some(saved) = self.checkpoint("PLANNING", &job.id) {
            saved
        } else {
            let generated = self
                .measure_and_log(&job.id, job.status, "plan", "llm-gateway:genius", || {
                    self.llm.plan(&planning_prompt)
                })
                .await?;
            self.checkpoints
                .lock()
                .expect("checkpoint lock")
                .save(&job.id, "PLANNING", &generated);
            generated
        };
        self.evidence.write(&job.id, "plan", &plan)?;
        let governance_plan =
            match validate_plan_governance(&plan, self.pr_policy.plan_governance()) {
                Ok(value) => value,
                Err(err) => {
                    self.queue_linear_comment(
                        &issue.id,
                        format!(
                            "`code247:plan-invalid` run_id=`{}` reason=`{}`",
                            job.id,
                            err.to_string().replace('`', "'")
                        ),
                    )
                    .await;
                    return Err(err);
                }
            };
        self.evidence.write(
            &job.id,
            "plan_contract",
            &serde_json::to_string_pretty(&json!({
                "objective_present": governance_plan.objective_present,
                "changes_present": governance_plan.changes_present,
                "risk_present": governance_plan.risk_present,
                "acceptance": governance_plan.acceptance,
                "how_to_test": governance_plan.how_to_test,
                "backout": governance_plan.backout,
            }))?,
        )?;
        self.evidence.write(
            &job.id,
            "acceptance",
            &serde_json::to_string_pretty(&json!({
                "items": governance_plan.acceptance,
            }))?,
        )?;
        self.evidence.write(
            &job.id,
            "how_to_test",
            &serde_json::to_string_pretty(&json!({
                "items": governance_plan.how_to_test,
            }))?,
        )?;
        self.evidence.write(
            &job.id,
            "backout",
            &serde_json::to_string_pretty(&json!({
                "items": governance_plan.backout,
            }))?,
        )?;

        self.transition(&mut job, JobStatus::Coding)?;
        let mut code = if let Some(saved) = self.checkpoint("CODING", &job.id) {
            saved
        } else {
            let generated = self
                .measure_and_log(&job.id, job.status, "code", "llm-gateway:code", || {
                    self.llm.code(&plan)
                })
                .await?;
            self.checkpoints
                .lock()
                .expect("checkpoint lock")
                .save(&job.id, "CODING", &generated);
            generated
        };
        self.evidence.write(&job.id, "code", &code)?;

        self.transition(&mut job, JobStatus::Reviewing)?;
        let mut review = self
            .measure_and_log(&job.id, job.status, "review", "llm-gateway:genius", || {
                self.llm.review(&code)
            })
            .await?;

        let mut iteration = 0;
        while !review.issues.is_empty() && iteration < self.max_review_iterations {
            code = self
                .measure_and_log(&job.id, job.status, "recoding", "llm-gateway:code", || {
                    self.llm.code(&review.code)
                })
                .await?;
            review = self
                .measure_and_log(
                    &job.id,
                    job.status,
                    "rereview",
                    "llm-gateway:genius",
                    || self.llm.review(&code),
                )
                .await?;
            iteration += 1;
        }
        if !review.issues.is_empty() {
            let exhausted_payload = json!({
                "event": "REVIEW_LOOP_EXHAUSTED",
                "run_id": job.id,
                "iteration": iteration,
                "remaining_issues": review.issues,
                "summary": review.summary,
            });
            self.evidence.write(
                &job.id,
                "review_loop_exhausted",
                &serde_json::to_string_pretty(&exhausted_payload)?,
            )?;
            self.execution_logger
                .lock()
                .expect("logger lock")
                .log_stage(
                    &job.id,
                    "review_loop_exhausted",
                    "(review loop)",
                    &serde_json::to_string(&exhausted_payload)?,
                    "llm-gateway:genius",
                    0,
                );
            self.queue_linear_comment(
                &issue.id,
                format!(
                    "`code247:review-loop-exhausted` event=`REVIEW_LOOP_EXHAUSTED` run_id=`{}` iteration=`{}` remaining_issues=`{}`",
                    job.id,
                    iteration,
                    review.issues.len()
                ),
            )
            .await;
            bail!(
                "review loop exhausted after {} iteration(s); remaining issues: {}",
                iteration,
                review.issues.len()
            );
        }

        self.evidence
            .write(&job.id, "review", &serde_json::to_string_pretty(&review)?)?;

        self.file_writer.write_from_llm_output(&code)?;

        self.transition(&mut job, JobStatus::Validating)?;
        let validation = self.test_runner.validate().await?;
        self.evidence.write(
            &job.id,
            "validation",
            &serde_json::to_string_pretty(&validation)?,
        )?;
        if validation.red_main_blocked {
            self.queue_linear_comment(
                &issue.id,
                format!(
                    "`code247:red-main` run_id=`{}` status=`blocked` reason=`main-not-green`",
                    job.id
                ),
            )
            .await;
        }
        if validation.flaky_recovered {
            self.queue_linear_comment(
                &issue.id,
                format!(
                    "`code247:ci-flaky` run_id=`{}` status=`recovered-after-rerun`",
                    job.id
                ),
            )
            .await;
        }
        if !validation.passed {
            bail!("validação falhou: {}", validation.errors.join("; "));
        }

        let files = self.git.changed_files().await?;

        self.transition(&mut job, JobStatus::Committing)?;
        let commit = self
            .git
            .commit(&job.id, "auto-commit", &files, &review.summary)
            .await?;
        self.execution_logger
            .lock()
            .expect("logger lock")
            .log_stage(
                &job.id,
                "commit",
                &serde_json::to_string(&files)?,
                &serde_json::to_string(&commit)?,
                "git",
                0,
            );
        let diff_lines = self.git.diff_lines_for_commit(&commit.sha).await?;
        let risk = RiskClassifier::classify(&files, diff_lines);
        self.queue_linear_comment(
            &issue.id,
            format!(
                "`code247:plan` run_id=`{}` merge_mode=`{:?}` risk_score=`{}` diff_lines=`{}` changed_files=`{}`",
                job.id, risk.merge_mode, risk.score, risk.diff_lines, risk.changed_files
            ),
        )
        .await;
        self.evidence
            .write(&job.id, "risk", &serde_json::to_string_pretty(&risk)?)?;
        self.execution_logger
            .lock()
            .expect("logger lock")
            .log_stage(
                &job.id,
                "risk",
                &serde_json::to_string(&files)?,
                &serde_json::to_string(&risk)?,
                "policy/risk-score:v1",
                0,
            );

        self.git.push_branch(&branch).await?;

        let pr_creator = self.pr_creator.as_ref().ok_or_else(|| {
            anyhow!("GitHub PR integration is required (missing GITHUB_TOKEN/GITHUB_REPO)")
        })?;
        let policy_meta = self.pr_policy.metadata();
        let cloud_gate = if risk.merge_mode == MergeMode::Substantial {
            let context = json!({
                "issue": {
                    "id": issue.id,
                    "identifier": issue.identifier,
                    "title": issue.title,
                },
                "risk": risk,
                "files": files,
                "review_summary": review.summary,
                "review_issues": review.issues,
                "validation": validation,
            });
            let decision = self
                .measure_and_log(
                    &job.id,
                    job.status,
                    "cloud_gate",
                    "llm-gateway:genius",
                    || self.llm.cloud_pr_risk_decision(context.clone()),
                )
                .await?;
            self.evidence.write(
                &job.id,
                "cloud_gate",
                &serde_json::to_string_pretty(&decision)?,
            )?;
            Some(decision)
        } else {
            None
        };
        let merge_policy = resolve_merge_policy_decision(
            &job.payload,
            &risk,
            cloud_gate.as_ref(),
            &self.pr_policy,
        );
        self.evidence.write(
            &job.id,
            "merge_policy_decision",
            &serde_json::to_string_pretty(&merge_policy)?,
        )?;
        self.execution_logger
            .lock()
            .expect("logger lock")
            .log_stage(
                &job.id,
                "merge_policy",
                &serde_json::to_string(&json!({
                    "merge_mode": merge_policy.merge_mode,
                    "risk_score": merge_policy.risk_score,
                    "override_requested": merge_policy.override_applied.is_some(),
                }))?,
                &serde_json::to_string(&merge_policy)?,
                "policy/merge:v1",
                0,
            );
        if let Some(policy_eval) = merge_policy.cloud_policy.as_ref() {
            self.evidence.write(
                &job.id,
                "cloud_gate_policy",
                &serde_json::to_string_pretty(policy_eval)?,
            )?;
        }
        if let Some(override_request) = merge_policy.override_applied.as_ref() {
            self.queue_linear_comment(
                &issue.id,
                format!(
                    "`code247:merge-override` run_id=`{}` action=`{:?}` actor=`{}` ticket=`{}` reason=`{}`",
                    job.id,
                    override_request.action,
                    override_request.actor,
                    override_request.ticket.as_deref().unwrap_or("-"),
                    override_request.reason.replace('`', "'"),
                ),
            )
            .await;
        }
        let mut pre_merge_required = vec![
            "plan",
            "plan_contract",
            "acceptance",
            "how_to_test",
            "backout",
            "code",
            "review",
            "validation",
            "risk",
            "merge_policy_decision",
        ];
        if risk.merge_mode == MergeMode::Substantial {
            pre_merge_required.push("cloud_gate");
        }
        self.enforce_evidence_gate(&issue.id, &job.id, "merge", &pre_merge_required)
            .await?;

        let checks_url = format!(
            "https://github.com/{}/commit/{}/checks",
            pr_creator.repo_slug(),
            commit.sha
        );
        let review_for_pr = ReviewOutput {
            summary: format!(
                "{}\n\n{}",
                review.summary,
                governance_plan.to_markdown_appendix(&checks_url)
            ),
            issues: review.issues.clone(),
            code: review.code.clone(),
        };

        let (number, url) = pr_creator
            .create(&job, &issue, &review_for_pr, &branch, &files, &risk)
            .await?;
        self.queue_linear_comment(
            &issue.id,
            format!(
                "`code247:pr-opened` run_id=`{}` pr=`#{}` {}",
                job.id, number, url
            ),
        )
        .await;
        self.evidence
            .write(&job.id, "pr", &format!("PR #{}: {}", number, url))?;
        self.evidence.write(
            &job.id,
            "checks_link",
            &serde_json::to_string_pretty(&json!({
                "pr_number": number,
                "pr_url": url,
                "checks_url": checks_url,
            }))?,
        )?;
        self.enforce_evidence_gate(
            &issue.id,
            &job.id,
            "auto-merge",
            &[
                "plan_contract",
                "acceptance",
                "how_to_test",
                "backout",
                "validation",
                "risk",
                "pr",
                "checks_link",
            ],
        )
        .await?;
        let merge = if merge_policy.auto_merge_allowed {
            pr_creator
                .auto_merge_when_ready(number, &risk, true)
                .await?
        } else {
            crate::pr_creator_rs::AutoMergeOutcome {
                attempted: false,
                merged: false,
                reason: merge_policy.reason.clone(),
                merge_commit_sha: None,
            }
        };
        self.evidence.write(
            &job.id,
            "merge",
            &serde_json::to_string_pretty(&json!({
                "attempted": merge.attempted,
                "merged": merge.merged,
                "reason": merge.reason,
                "merge_commit_sha": merge.merge_commit_sha,
                "merge_policy_resolution": merge_policy.resolution,
                "merge_policy_override": merge_policy.override_applied,
                "policy_version": policy_meta.version,
                "policy_sha256": policy_meta.source_sha256,
                "policy_path": policy_meta.source_path,
            }))?,
        )?;
        self.execution_logger
            .lock()
            .expect("logger lock")
            .log_stage(
                &job.id,
                "merge",
                &serde_json::to_string(&json!({ "pr_number": number, "pr_url": url }))?,
                &serde_json::to_string(&json!({
                    "attempted": merge.attempted,
                    "merged": merge.merged,
                    "reason": merge.reason,
                    "merge_commit_sha": merge.merge_commit_sha,
                    "merge_policy_resolution": merge_policy.resolution,
                    "merge_policy_override": merge_policy.override_applied,
                    "policy_version": policy_meta.version,
                    "policy_sha256": policy_meta.source_sha256,
                    "policy_path": policy_meta.source_path,
                }))?,
                "github-auto-merge",
                0,
            );

        match risk.merge_mode {
            MergeMode::Light => {
                if !merge_policy.auto_merge_allowed {
                    self.queue_linear_comment(
                        &issue.id,
                        format!(
                            "`code247:needs-human` run_id=`{}` reason=`light-manual-review` detail=`{}`",
                            job.id, merge_policy.reason
                        ),
                    )
                    .await;
                    bail!("light PR requires manual review: {}", merge_policy.reason);
                }
                if !merge.merged {
                    self.queue_linear_comment(
                        &issue.id,
                        format!(
                            "`code247:needs-human` run_id=`{}` reason=`light-merge-failed` detail=`{}`",
                            job.id, merge.reason
                        ),
                    )
                    .await;
                    bail!(
                        "light PR não foi mergeado automaticamente: {}",
                        merge.reason
                    );
                }
            }
            MergeMode::Substantial => {
                if !merge_policy.auto_merge_allowed {
                    if let Some(policy_eval) = merge_policy.cloud_policy.as_ref() {
                        self.queue_linear_comment(
                            &issue.id,
                            format!(
                                "`code247:needs-cloud-review` run_id=`{}` decision=`{}` reason=`{}`",
                                job.id, policy_eval.cloud_decision, policy_eval.reason
                            ),
                        )
                        .await;
                    } else {
                        self.queue_linear_comment(
                            &issue.id,
                            format!(
                                "`code247:needs-human` run_id=`{}` reason=`substantial-manual-review` detail=`{}`",
                                job.id, merge_policy.reason
                            ),
                        )
                        .await;
                    }
                    bail!(
                        "substantial PR blocked before auto-merge: {}",
                        merge_policy.reason
                    );
                }
                if !merge.merged {
                    self.queue_linear_comment(
                        &issue.id,
                        format!(
                            "`code247:needs-human` run_id=`{}` reason=`substantial-merge-failed` detail=`{}`",
                            job.id, merge.reason
                        ),
                    )
                    .await;
                    bail!(
                        "substantial PR cloud-approved mas merge não concluiu: {}",
                        merge.reason
                    );
                }
            }
        }

        let rollback_chain = json!({
            "chain_id": format!("rollback:{}:pr:{}", job.id, number),
            "strategy": "revert_pull_request",
            "pr_number": number,
            "pr_url": url,
            "merge_commit_sha": merge.merge_commit_sha.clone().unwrap_or(commit.sha.clone()),
            "run_id": job.id,
            "steps": [
                "git revert <merge_commit_sha>",
                "abrir PR de rollback",
                "aguardar checks obrigatórios",
                "mergear rollback PR",
            ],
        });
        self.evidence.write(
            &job.id,
            "rollback_chain",
            &serde_json::to_string_pretty(&rollback_chain)?,
        )?;
        let mut pre_state_required = vec![
            "plan",
            "code",
            "review",
            "validation",
            "risk",
            "plan_contract",
            "acceptance",
            "how_to_test",
            "backout",
            "pr",
            "checks_link",
            "merge",
            "rollback_chain",
        ];
        if risk.merge_mode == MergeMode::Substantial {
            pre_state_required.push("cloud_gate_policy");
        }
        self.enforce_evidence_gate(
            &issue.id,
            &job.id,
            "linear-state-transition",
            &pre_state_required,
        )
        .await?;
        self.queue_linear_comment(
            &issue.id,
            format!(
                "`code247:validated` run_id=`{}` pr=`#{}` merged=`true` merge_commit_sha=`{}` rollback_chain_id=`{}` target_state=`{}`",
                job.id,
                number,
                rollback_chain["merge_commit_sha"].as_str().unwrap_or("unknown"),
                rollback_chain["chain_id"].as_str().unwrap_or("unknown"),
                self.linear_ready_for_release_state_name,
            ),
        )
        .await;
        self.queue_linear_state_transition(&issue.id, &self.linear_ready_for_release_state_name)
            .await?;

        self.transition(&mut job, JobStatus::Done)?;
        Ok(())
    }

    fn checkpoint(&self, stage: &str, job_id: &str) -> Option<String> {
        self.checkpoints
            .lock()
            .expect("checkpoint lock")
            .get_latest(job_id, stage)
    }

    fn transition(&self, job: &mut Job, to: JobStatus) -> Result<()> {
        if job.status == to {
            return Ok(());
        }
        if !self.fsm.can_transition(job.status, to) {
            return Err(anyhow!("Invalid transition {:?} -> {:?}", job.status, to));
        }
        let changed = self
            .jobs
            .lock()
            .expect("jobs lock")
            .transition_status_with_lease(
                &job.id,
                job.status,
                to,
                None,
                Some(&self.lease_owner),
                self.stage_timeout_seconds(to),
            );
        if !changed {
            return Err(anyhow!(
                "job {} lost stage state/lease while transitioning {:?} -> {:?}",
                job.id,
                job.status,
                to
            ));
        }
        job.status = to;
        Ok(())
    }

    async fn measure_and_log<T, F, Fut>(
        &self,
        job_id: &str,
        current_status: JobStatus,
        stage: &str,
        model: &str,
        f: F,
    ) -> Result<T>
    where
        T: serde::Serialize,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let start = Instant::now();
        let result = f().await?;
        let duration = start.elapsed().as_millis() as i64;
        self.execution_logger
            .lock()
            .expect("logger lock")
            .log_stage(
                job_id,
                stage,
                "(see checkpoints)",
                &serde_json::to_string(&result)?,
                model,
                duration,
            );
        if current_status.has_stage_lease() {
            let renewed = self.jobs.lock().expect("jobs lock").renew_stage_lease(
                job_id,
                current_status,
                &self.lease_owner,
                self.stage_timeout_seconds(current_status).unwrap_or(300),
            );
            if !renewed {
                bail!(
                    "job {} perdeu lease da etapa {:?} durante '{}'",
                    job_id,
                    current_status,
                    stage
                );
            }
        }
        Ok(result)
    }

    fn stage_timeout_seconds(&self, status: JobStatus) -> Option<i64> {
        match status {
            JobStatus::Planning => Some(self.planning_timeout_seconds),
            JobStatus::Coding => Some(self.coding_timeout_seconds),
            JobStatus::Reviewing => Some(self.reviewing_timeout_seconds),
            JobStatus::Validating => Some(self.validating_timeout_seconds),
            JobStatus::Committing => Some(self.committing_timeout_seconds),
            _ => None,
        }
    }

    async fn queue_linear_comment(&self, issue_id: &str, body: String) {
        if let Err(err) = self
            .linear_outbox
            .lock()
            .expect("linear outbox lock")
            .enqueue(issue_id, "comment", &json!({ "body": body }))
        {
            warn!(issue_id=%issue_id, error=%err, "failed to enqueue linear comment");
        }
    }

    async fn enforce_evidence_gate(
        &self,
        issue_id: &str,
        job_id: &str,
        gate: &str,
        required: &[&str],
    ) -> Result<()> {
        let missing = self.evidence.missing_stages(job_id, required);
        if missing.is_empty() {
            return Ok(());
        }
        self.queue_linear_comment(
            issue_id,
            format!(
                "`code247:evidence-missing` run_id=`{}` gate=`{}` missing=`{}`",
                job_id,
                gate,
                missing.join(",")
            ),
        )
        .await;
        bail!(
            "fail-closed: evidência obrigatória ausente para gate '{}': {}",
            gate,
            missing.join(", ")
        );
    }

    async fn queue_linear_state_transition(&self, issue_id: &str, state_name: &str) -> Result<()> {
        let target_name = state_name.trim();
        if target_name.is_empty() {
            return Ok(());
        }
        if target_name.eq_ignore_ascii_case(&self.linear_in_progress_state_name)
            || target_name.eq_ignore_ascii_case(&self.linear_ready_for_release_state_name)
        {
            let current_issue = self.linear.get_issue(issue_id).await?;
            let current_state = classify_linear_workflow_state(
                &current_issue.state.name,
                &current_issue.state.r#type,
                "Ready",
                &self.linear_in_progress_state_name,
                &self.linear_ready_for_release_state_name,
                &self.linear_done_state_type,
            );
            let target_state =
                if target_name.eq_ignore_ascii_case(&self.linear_in_progress_state_name) {
                    LinearWorkflowState::InProgress
                } else {
                    LinearWorkflowState::ReadyForRelease
                };
            if !is_linear_transition_allowed(current_state, target_state) {
                bail!(
                    "linear transition blocked by guard: '{}' -> '{}'",
                    current_issue.state.name,
                    target_name
                );
            }
        }
        self.linear_outbox
            .lock()
            .expect("linear outbox lock")
            .enqueue(
                issue_id,
                "transition",
                &json!({ "state_name": target_name }),
            )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_merge_policy_override, resolve_merge_policy_decision, validate_plan_governance,
    };
    use crate::{
        adapters_rs::CloudGateDecision,
        policy_gate_rs::{PlanGovernancePolicy, PrRiskPolicy},
        risk_classifier_rs::{MergeMode, RiskAssessment},
    };

    #[test]
    fn accepts_plan_with_required_governance_sections() {
        let plan = r#"
Objetivo: reduzir latência no endpoint de claims.

Mudanças:
- src/api_rs.rs
- src/pipeline_rs.rs

Risco: MEDIUM por impacto em fluxo de transição.

Acceptance Criteria:
- dado claim válido, quando sync roda, então a issue vai para In Progress.
- dado transição inválida, quando sync roda, então bloqueia com erro auditável.

How To Test:
- cargo test
- ./scripts/smoke-p1-state-governance.sh

Backout:
- revert do commit em caso de regressão
"#;

        let parsed = validate_plan_governance(plan, &PlanGovernancePolicy::default_fail_closed())
            .expect("plan should be valid");
        assert!(parsed.objective_present);
        assert!(parsed.changes_present);
        assert!(parsed.risk_present);
        assert_eq!(parsed.acceptance.len(), 2);
        assert_eq!(parsed.how_to_test.len(), 2);
        assert_eq!(parsed.backout.len(), 1);
    }

    #[test]
    fn rejects_plan_missing_backout_section() {
        let plan = r#"
Goal: stabilize merge policy.

Changes:
- src/pr_creator_rs.rs

Risk: LOW because only metadata changed.

Acceptance:
- merge light works after checks green.

How to test:
- cargo test
"#;

        let err = validate_plan_governance(plan, &PlanGovernancePolicy::default_fail_closed())
            .expect_err("plan should be rejected");
        let message = err.to_string();
        assert!(message.contains("backout/rollback"), "{message}");
    }

    #[test]
    fn accepts_plan_when_policy_relaxes_backout_requirement() {
        let plan = r#"
Goal: stabilize merge policy.

Changes:
- src/pr_creator_rs.rs

Risk: LOW because only metadata changed.

Acceptance:
- merge light works after checks green.

How to test:
- cargo test
"#;

        let policy = PlanGovernancePolicy {
            require_objective: true,
            require_changes: true,
            require_risk: true,
            require_acceptance: true,
            require_how_to_test: true,
            require_backout: false,
        };
        let parsed = validate_plan_governance(plan, &policy).expect("plan should be accepted");
        assert_eq!(parsed.acceptance.len(), 1);
    }

    #[test]
    fn parses_merge_override_from_payload() {
        let payload = r#"{
          "merge_policy": {
            "override": {
              "action": "allow_auto_merge",
              "actor": "ops@example.com",
              "reason": "incident mitigation",
              "ticket": "OPS-123",
              "source": "manual",
              "approved_at": "2026-03-06T15:00:00Z"
            }
          }
        }"#;

        let parsed = parse_merge_policy_override(payload).expect("override should parse");
        assert_eq!(parsed.actor, "ops@example.com");
        assert_eq!(parsed.reason, "incident mitigation");
        assert_eq!(parsed.ticket.as_deref(), Some("OPS-123"));
    }

    #[test]
    fn force_manual_override_blocks_light_auto_merge() {
        let payload = r#"{
          "controls": {
            "merge_policy": {
              "override": {
                "action": "force_manual_review",
                "actor": "reviewer@example.com",
                "reason": "needs human eyes"
              }
            }
          }
        }"#;
        let risk = RiskAssessment {
            score: 1,
            merge_mode: MergeMode::Light,
            diff_lines: 20,
            changed_files: 1,
            changed_modules: 1,
            docs_only: false,
            tests_touched: true,
            sensitive_paths: vec![],
            reason_codes: vec![],
        };

        let decision = resolve_merge_policy_decision(
            payload,
            &risk,
            None,
            &PrRiskPolicy::load_from_path("missing", false).unwrap(),
        );
        assert!(!decision.auto_merge_allowed);
        assert_eq!(
            decision.resolution,
            super::MergePolicyResolution::ManualReview
        );
        assert!(decision.reason.contains("manual review forced"));
    }

    #[test]
    fn allow_auto_merge_override_unblocks_substantial_cloud_denial() {
        let payload = r#"{
          "merge_policy": {
            "override": {
              "action": "allow_auto_merge",
              "actor": "lead@example.com",
              "reason": "approved emergency patch",
              "ticket": "SEC-9"
            }
          }
        }"#;
        let risk = RiskAssessment {
            score: 5,
            merge_mode: MergeMode::Substantial,
            diff_lines: 240,
            changed_files: 4,
            changed_modules: 4,
            docs_only: false,
            tests_touched: false,
            sensitive_paths: vec!["src/auth.rs".to_string()],
            reason_codes: vec!["touches_sensitive_paths".to_string()],
        };
        let gate = CloudGateDecision {
            decision: "NO".to_string(),
            confidence: 0.42,
            reason_codes: vec!["secrets_suspected".to_string()],
            rationale: "needs manual validation".to_string(),
        };

        let decision = resolve_merge_policy_decision(
            payload,
            &risk,
            Some(&gate),
            &PrRiskPolicy::load_from_path("missing", false).unwrap(),
        );
        assert!(decision.auto_merge_allowed);
        assert_eq!(decision.resolution, super::MergePolicyResolution::AutoMerge);
        assert!(decision.reason.contains("override approved"));
        assert!(decision.override_applied.is_some());
        assert!(decision.cloud_policy.is_some());
    }
}
