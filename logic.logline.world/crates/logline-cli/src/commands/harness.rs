use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Subcommand)]
pub enum HarnessCommands {
    /// Run the unified severe integration pipeline and write an auditable report.
    Run {
        /// Integration root path (contains TASKLIST-GERAL.md and scripts/)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Output report path (default: <root>/artifacts/integration-severe-report.json)
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Replay a previous severe integration report (all scenarios or only failed ones).
    Replay {
        /// Integration root path (contains TASKLIST-GERAL.md and scripts/)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Existing report to replay
        #[arg(long)]
        report: PathBuf,
        /// Replay only failed scenarios from the report
        #[arg(long)]
        failed_only: bool,
    },
    /// Generate cookbook from the canonical capability catalog.
    Cookbook {
        /// Integration root path (contains scripts/generate-cookbook.mjs)
        #[arg(long)]
        root: Option<PathBuf>,
        /// Optional output markdown path
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Standardized intentions flow (generate/publish/sync/replay).
    Intentions {
        #[command(subcommand)]
        command: IntentionsCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum IntentionsCommands {
    /// Generate a canonical manifest.intentions.json for logic/CLI release flow.
    Generate {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "logic.logline.world")]
        workspace: String,
        #[arg(long, default_value = "logic-cli")]
        project: String,
    },
    /// Publish manifest.intentions.json through Code247 intake and persist linkage meta.
    Publish {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long, default_value = "code247-ci/main")]
        ci_target: String,
        #[arg(long)]
        meta_output: Option<PathBuf>,
    },
    /// Sync execution status to Code247/Linear using a payload file.
    Sync {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        payload: PathBuf,
        #[arg(long)]
        meta_output: Option<PathBuf>,
    },
    /// Replay sync from a previous harness report (requires sync payload path in report).
    Replay {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        report: PathBuf,
        #[arg(long)]
        meta_output: Option<PathBuf>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandRecord {
    argv: Vec<String>,
    cwd: String,
    env: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExecutionRecord {
    id: String,
    title: String,
    applicable: bool,
    status: String,
    elapsed_ms: u128,
    command: Option<CommandRecord>,
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    stderr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HarnessReport {
    report_version: String,
    request_id: String,
    generated_at: String,
    root: String,
    pipeline: String,
    artifacts: HarnessArtifacts,
    steps: Vec<ExecutionRecord>,
    scenarios: Vec<ExecutionRecord>,
    round_trip: Value,
    summary: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct HarnessArtifacts {
    json_report: String,
    markdown_summary: String,
    tasklist_general: String,
    capability_catalog: String,
    capability_cookbook: String,
    smoke_script: String,
    contracts_script: String,
    severe_suite_script: String,
    logic_linear_meta: String,
    code247_linear_meta: String,
    root_code247_dir: String,
    code247_evidence_dir: String,
    code247_sqlite_db: String,
}

#[derive(Debug, Clone)]
struct ScenarioDefinition {
    id: &'static str,
    title: &'static str,
    argv: Vec<String>,
    env: Vec<(&'static str, &'static str)>,
}

#[derive(Debug, Default)]
struct JsonLoadResult {
    exists: bool,
    parse_ok: bool,
    value: Option<Value>,
    error: Option<String>,
}

pub fn cmd_harness(command: HarnessCommands, json: bool) -> Result<()> {
    match command {
        HarnessCommands::Run { root, report } => cmd_harness_run(root, report, json),
        HarnessCommands::Replay {
            root,
            report,
            failed_only,
        } => cmd_harness_replay(root, &report, failed_only, json),
        HarnessCommands::Cookbook { root, output } => cmd_harness_cookbook(root, output, json),
        HarnessCommands::Intentions { command } => cmd_harness_intentions(command, json),
    }
}

fn cmd_harness_run(root: Option<PathBuf>, report: Option<PathBuf>, json: bool) -> Result<()> {
    let root = resolve_root(root)?;
    let request_id = format!("logic-harness-{}", Uuid::new_v4());
    let report_path = report.unwrap_or_else(|| {
        root.join("artifacts")
            .join("integration-severe-report.json")
    });
    let markdown_path = report_path.with_extension("md");

    let mut steps = Vec::new();
    steps.push(run_external_step(
        "STEP-000",
        "Smoke gate (script integrity)",
        &root,
        &[
            "bash".to_string(),
            "-n".to_string(),
            "scripts/smoke.sh".to_string(),
        ],
        &[],
    ));
    steps.push(run_external_step(
        "STEP-001",
        "Sync Canon schemas + generated TypeScript check",
        &root,
        &[
            "bash".to_string(),
            "scripts/sync-canon-schemas.sh".to_string(),
            "--check".to_string(),
        ],
        &[],
    ));
    steps.push(run_external_step(
        "STEP-002",
        "Generate cookbook from canonical capability catalog",
        &root,
        &[
            "node".to_string(),
            "scripts/generate-cookbook.mjs".to_string(),
        ],
        &[],
    ));
    steps.push(run_external_step(
        "STEP-003",
        "Validate contracts/policy/openapi globally",
        &root,
        &[
            "bash".to_string(),
            "scripts/validate-contracts.sh".to_string(),
        ],
        &[],
    ));

    let scenarios = run_integration_severe_suite(&root);
    let round_trip = collect_round_trip_snapshot(&root);

    let step_failures = steps.iter().filter(|item| item.status == "failed").count();
    let scenario_failures = scenarios
        .iter()
        .filter(|item| item.applicable && item.status == "failed")
        .count();
    let scenario_skipped = scenarios
        .iter()
        .filter(|item| item.status == "skipped")
        .count();

    let report_payload = HarnessReport {
        report_version: "logic.integration-severe.report.v1".to_string(),
        request_id: request_id.clone(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        root: root.display().to_string(),
        pipeline: "smoke+contracts+integration-severe+report".to_string(),
        artifacts: collect_harness_artifacts(&root, &report_path, &markdown_path),
        steps,
        scenarios,
        round_trip: round_trip.clone(),
        summary: json!({
            "ok": step_failures == 0 && scenario_failures == 0,
            "step_failures": step_failures,
            "scenario_failures": scenario_failures,
            "scenario_skipped": scenario_skipped,
            "round_trip": {
                "intentions_total": round_trip["intentions_total"].as_u64().unwrap_or(0),
                "linked_total": round_trip["linked_total"].as_u64().unwrap_or(0),
                "synced_total": round_trip["synced_total"].as_u64().unwrap_or(0),
                "sync_errors_total": round_trip["sync_errors_total"].as_u64().unwrap_or(0),
            }
        }),
    };

    write_json_file(&report_path, &report_payload)?;
    write_text_file(&markdown_path, &render_markdown_summary(&report_payload))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report_payload)?);
    } else {
        println!("Integration severe pipeline completed.");
        println!("request_id: {request_id}");
        println!("report: {}", report_path.display());
        println!("summary_markdown: {}", markdown_path.display());
        println!(
            "summary: ok={} step_failures={} scenario_failures={} skipped={}",
            report_payload.summary["ok"].as_bool().unwrap_or(false),
            report_payload.summary["step_failures"]
                .as_u64()
                .unwrap_or(0),
            report_payload.summary["scenario_failures"]
                .as_u64()
                .unwrap_or(0),
            report_payload.summary["scenario_skipped"]
                .as_u64()
                .unwrap_or(0)
        );
    }

    if !report_payload.summary["ok"].as_bool().unwrap_or(false) {
        bail!(
            "integration-severe failed (step_failures={}, scenario_failures={})",
            report_payload.summary["step_failures"],
            report_payload.summary["scenario_failures"],
        );
    }

    Ok(())
}

fn cmd_harness_replay(
    root: Option<PathBuf>,
    report_path: &Path,
    failed_only: bool,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root)?;
    let raw = fs::read_to_string(report_path)
        .with_context(|| format!("failed to read report {}", report_path.display()))?;
    let existing: HarnessReport = serde_json::from_str(&raw)
        .with_context(|| format!("invalid report JSON {}", report_path.display()))?;

    let mut scenarios_to_run = Vec::new();
    for scenario in &existing.scenarios {
        if !scenario.applicable {
            continue;
        }
        if failed_only && scenario.status != "failed" {
            continue;
        }
        let Some(command) = scenario.command.as_ref() else {
            continue;
        };
        scenarios_to_run.push(ScenarioDefinition {
            id: Box::leak(scenario.id.clone().into_boxed_str()),
            title: Box::leak(scenario.title.clone().into_boxed_str()),
            argv: command.argv.clone(),
            env: command
                .env
                .iter()
                .map(|(k, v)| {
                    let key = Box::leak(k.clone().into_boxed_str());
                    let value = Box::leak(v.clone().into_boxed_str());
                    (key as &'static str, value as &'static str)
                })
                .collect(),
        });
    }

    let replay_results = scenarios_to_run
        .iter()
        .map(|scenario| run_scenario(&root, scenario))
        .collect::<Vec<_>>();
    let replay_failures = replay_results
        .iter()
        .filter(|item| item.status == "failed")
        .count();

    let replay_report = json!({
        "report_version": "logic.integration-severe.replay.v1",
        "source_report": report_path,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "failed_only": failed_only,
        "ok": replay_failures == 0,
        "scenario_failures": replay_failures,
        "scenarios": replay_results,
    });

    let output_path = report_path.with_extension("replay.json");
    write_json_file(&output_path, &replay_report)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&replay_report)?);
    } else {
        println!("Replay complete. report: {}", output_path.display());
    }

    if replay_failures > 0 {
        bail!("replay failed with {replay_failures} scenario(s)");
    }

    Ok(())
}

fn cmd_harness_cookbook(root: Option<PathBuf>, output: Option<PathBuf>, json: bool) -> Result<()> {
    let root = resolve_root(root)?;
    let mut argv = vec![
        "node".to_string(),
        "scripts/generate-cookbook.mjs".to_string(),
    ];
    if let Some(path) = output.as_ref() {
        argv.push(path.display().to_string());
    }
    let result = run_external_step(
        "COOKBOOK",
        "Generate cookbook from canonical catalog",
        &root,
        &argv,
        &[],
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!(
            "Cookbook generation status: {} ({})",
            result.status, result.title
        );
    }

    if result.status != "ok" {
        bail!("cookbook generation failed");
    }
    Ok(())
}

fn cmd_harness_intentions(command: IntentionsCommands, json: bool) -> Result<()> {
    match command {
        IntentionsCommands::Generate {
            root,
            output,
            workspace,
            project,
        } => cmd_intentions_generate(root, output, &workspace, &project, json),
        IntentionsCommands::Publish {
            root,
            manifest,
            ci_target,
            meta_output,
        } => cmd_intentions_publish(root, manifest, &ci_target, meta_output, json),
        IntentionsCommands::Sync {
            root,
            payload,
            meta_output,
        } => cmd_intentions_sync(root, &payload, meta_output, json),
        IntentionsCommands::Replay {
            root,
            report,
            meta_output,
        } => cmd_intentions_replay(root, &report, meta_output, json),
    }
}

fn cmd_intentions_generate(
    root: Option<PathBuf>,
    output: Option<PathBuf>,
    workspace: &str,
    project: &str,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root)?;
    let output_path = output.unwrap_or_else(|| {
        root.join("logic.logline.world")
            .join(".code247")
            .join("manifest.intentions.json")
    });

    let payload = json!({
        "workspace": workspace,
        "project": project,
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "intentions": [
            {
                "id": "logic-agent-c-integration-severe",
                "title": "Logic/CLI integration severe harness",
                "type": "hardening",
                "scope": "backend",
                "priority": "high",
                "tasks": [
                    {"description": "LOGIC-001 consolidate CLI/runtime intentions+sync+replay", "owner": "agent-c", "gate": "harness"},
                    {"description": "LOGIC-002 contracts policy/gates consumable sem adapters ad-hoc", "owner": "agent-c", "gate": "contracts"},
                    {"description": "LOGIC-003 output estável para auditoria/replay", "owner": "agent-c", "gate": "report"},
                    {"description": "LOGIC-007 publish/sync linkage release flow", "owner": "agent-c", "gate": "intentions"},
                    {"description": "LOGIC-008 cookbook auto do catálogo canônico", "owner": "agent-c", "gate": "catalog"},
                    {"description": "LOGIC-009 hardening auth policy + schema updates", "owner": "agent-c", "gate": "auth"},
                    {"description": "LOGIC-010 sync Canon AST -> schema/TypeScript", "owner": "agent-c", "gate": "schema"}
                ]
            }
        ]
    });

    write_json_file(&output_path, &payload)?;
    crate::pout(
        json,
        json!({
            "ok": true,
            "manifest": output_path,
        }),
        &format!("Manifest generated: {}", output_path.display()),
    )
}

fn cmd_intentions_publish(
    root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    ci_target: &str,
    meta_output: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root)?;
    let manifest_path = manifest.unwrap_or_else(|| {
        root.join("logic.logline.world")
            .join(".code247")
            .join("manifest.intentions.json")
    });
    if !manifest_path.exists() {
        cmd_intentions_generate(
            None,
            Some(manifest_path.clone()),
            "logic.logline.world",
            "logic-cli",
            true,
        )?;
    }

    let meta_path = meta_output.unwrap_or_else(|| {
        root.join("logic.logline.world")
            .join(".code247")
            .join("linear-meta.json")
    });
    let argv = vec![
        "bash".to_string(),
        "scripts/publish-intentions.sh".to_string(),
        manifest_path.display().to_string(),
        ci_target.to_string(),
        meta_path.display().to_string(),
    ];
    let result = run_external_step(
        "INTENTIONS-PUBLISH",
        "Publish intentions",
        &root,
        &argv,
        &[],
    );
    if result.status != "ok" {
        bail!(
            "intentions publish failed: {}",
            result.error.unwrap_or_else(|| "unknown error".to_string())
        );
    }

    crate::pout(
        json,
        json!({
            "ok": true,
            "manifest": manifest_path,
            "meta_output": meta_path,
        }),
        &format!("Intentions published. linkage: {}", meta_path.display()),
    )
}

fn cmd_intentions_sync(
    root: Option<PathBuf>,
    payload: &Path,
    meta_output: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root)?;
    if !payload.exists() {
        bail!("sync payload not found: {}", payload.display());
    }

    let meta_path = meta_output.unwrap_or_else(|| {
        root.join("logic.logline.world")
            .join(".code247")
            .join("linear-meta.json")
    });

    let argv = vec![
        "bash".to_string(),
        "scripts/sync-intentions.sh".to_string(),
        payload.display().to_string(),
        meta_path.display().to_string(),
    ];
    let result = run_external_step("INTENTIONS-SYNC", "Sync intentions", &root, &argv, &[]);
    if result.status != "ok" {
        bail!(
            "intentions sync failed: {}",
            result.error.unwrap_or_else(|| "unknown error".to_string())
        );
    }

    crate::pout(
        json,
        json!({
            "ok": true,
            "payload": payload,
            "meta_output": meta_path,
        }),
        &format!("Intentions synced. linkage: {}", meta_path.display()),
    )
}

fn cmd_intentions_replay(
    root: Option<PathBuf>,
    report: &Path,
    meta_output: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root)?;
    let raw = fs::read_to_string(report)
        .with_context(|| format!("failed to read report {}", report.display()))?;
    let parsed: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON report {}", report.display()))?;

    let payload_path = parsed
        .pointer("/intentions/sync_payload")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("report does not contain intentions.sync_payload"))?;

    cmd_intentions_sync(Some(root), &payload_path, meta_output, json)
}

fn run_integration_severe_suite(root: &Path) -> Vec<ExecutionRecord> {
    integration_scenarios()
        .into_iter()
        .map(|scenario| run_scenario(root, &scenario))
        .collect()
}

fn integration_scenarios() -> Vec<ScenarioDefinition> {
    vec![
        ScenarioDefinition {
            id: "TST-001",
            title: "Smoke script integrity for full stack checks",
            argv: vec!["bash".to_string(), "-n".to_string(), "scripts/smoke.sh".to_string()],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-002",
            title: "Done transition blocked without canonical path",
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-q".to_string(),
                "--manifest-path".to_string(),
                "code247.logline.world/Cargo.toml".to_string(),
                "canonical_done_path_requires_ready_for_release".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-003",
            title: "Webhook integrity signature validation",
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-q".to_string(),
                "--manifest-path".to_string(),
                "code247.logline.world/Cargo.toml".to_string(),
                "validates_linear_signature_hex".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-004",
            title: "Gateway resilience baseline (compile gate)",
            argv: vec![
                "cargo".to_string(),
                "check".to_string(),
                "--manifest-path".to_string(),
                "llm-gateway.logline.world/Cargo.toml".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-005",
            title: "Red-main policy guard exists in runner path",
            argv: vec![
                "rg".to_string(),
                "-n".to_string(),
                "red-main|red_main".to_string(),
                "code247.logline.world/src/test_runner_rs.rs".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-006",
            title: "Flaky retry policy guard exists in runner path",
            argv: vec![
                "rg".to_string(),
                "-n".to_string(),
                "flaky|retry|rerun".to_string(),
                "code247.logline.world/src/test_runner_rs.rs".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-007",
            title: "Fuel baseline migration guard present",
            argv: vec![
                "rg".to_string(),
                "-n".to_string(),
                "fuel_window_baseline|baseline".to_string(),
                "logic.logline.world/supabase/migrations/20260306000016_fuel_window_baseline.sql"
                    .to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-008",
            title: "Auth hardening regression gate (logline-auth tests)",
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-q".to_string(),
                "--manifest-path".to_string(),
                "logic.logline.world/Cargo.toml".to_string(),
                "-p".to_string(),
                "logline-auth".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-009",
            title: "Fuel reconciliation drift contracts present",
            argv: vec![
                "rg".to_string(),
                "-n".to_string(),
                "usd_settled|usd_estimated|drift".to_string(),
                "logic.logline.world/supabase/migrations/20260305000012_fuel_l0_reconciler.sql"
                    .to_string(),
                "logic.logline.world/supabase/migrations/20260305000014_fuel_reconciler_rpc_security.sql"
                    .to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-010",
            title: "Webhook timestamp robustness under malformed payloads",
            argv: vec![
                "cargo".to_string(),
                "test".to_string(),
                "-q".to_string(),
                "--manifest-path".to_string(),
                "code247.logline.world/Cargo.toml".to_string(),
                "extracts_webhook_timestamp_number_or_string".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-011",
            title: "Edge-control JWKS path + idempotency persistence (restart/multi-instance)",
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "cargo test -q --manifest-path edge-control.logline.world/Cargo.toml draft_intention_returns_contract_and_rejects_duplicate_idempotency && cargo test -q --manifest-path edge-control.logline.world/Cargo.toml supabase_store_uses_shared_rpc_contract && rg -n \"decode_supabase_jwks|supabase_jwks_url\" edge-control.logline.world/src/auth.rs edge-control.logline.world/src/config.rs".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-012",
            title: "Code247 resilience under Linear/GitHub intermitência preserves queue/timeline",
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "cargo test -q --manifest-path code247.logline.world/Cargo.toml retries_transient_http_failures && cargo test -q --manifest-path code247.logline.world/Cargo.toml claim_next_pending_with_lease_is_atomic_across_instances && cargo test -q --manifest-path code247.logline.world/Cargo.toml sync_http_moves_in_progress_to_ready_for_release".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-013",
            title: "Fuel policy_version segmented by tenant/app without cross-tenant mixing",
            argv: vec![
                "rg".to_string(),
                "-n".to_string(),
                "tenant_id|app_id|policy_version".to_string(),
                "logic.logline.world/supabase/migrations/20260306000019_fuel_policy_alerts_ops.sql"
                    .to_string(),
                "logic.logline.world/supabase/migrations/20260307000022_fix_fuel_ops_materialize.sql"
                    .to_string(),
                "logic.logline.world/supabase/migrations/20260307000023_fix_fuel_ops_materialize_aliases.sql"
                    .to_string(),
                "obs-api.logline.world/lib/obs/fuel.ts".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-014",
            title: "OBS API security regression guard (challenge leakage/replay + user keys membership)",
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "rg -n \"requireAccess\\(req, 'read'\\)|requireJwtSubject|hasAppMembership\\(|tenant_id mismatch|app scope mismatch|User is not a member of this tenant/app\" 'obs-api.logline.world/app/api/v1/apps/[appId]/keys/user/route.ts' && rg -n \"status: 'pending'|expires_at|approved_at|session_token|cleanupExpiredCliChallenges|sanitizeCliChallenge\" 'obs-api.logline.world/app/api/v1/cli/auth/challenge/[challengeId]/approve/route.ts'".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-015",
            title: "tenant/resolve anti-enumeration + challenge TTL/rate-limit server-side",
            argv: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "rg -n \"requireJwtSubject|getUserFromAuthHeader|tenantMemberships|Tenant not found\" 'obs-api.logline.world/app/api/v1/auth/tenant/resolve/route.ts' && rg -n \"CHALLENGE_TTL_SECONDS|RATE_LIMIT_WINDOW_SECONDS|RATE_LIMIT_MAX_REQUESTS|too_many_requests|resolveChallengeExpiry|enforceChallengeCreateRateLimit|cleanupExpiredCliChallenges|RATE_LIMITED\" 'obs-api.logline.world/app/api/v1/cli/auth/challenge/route.ts' 'obs-api.logline.world/lib/auth/cli-challenge.ts'".to_string(),
            ],
            env: vec![],
        },
        ScenarioDefinition {
            id: "TST-GATE-004",
            title: "Gate: sensitive integration deltas require severe suite update",
            argv: vec![
                "bash".to_string(),
                "scripts/enforce-severe-gate.sh".to_string(),
            ],
            env: vec![],
        },
    ]
}

fn run_scenario(root: &Path, scenario: &ScenarioDefinition) -> ExecutionRecord {
    run_external_step(
        scenario.id,
        scenario.title,
        root,
        &scenario.argv,
        &scenario.env,
    )
}

fn run_external_step(
    id: &str,
    title: &str,
    root: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> ExecutionRecord {
    let started = Instant::now();
    let mut env_map = BTreeMap::new();
    for (k, v) in env {
        env_map.insert((*k).to_string(), (*v).to_string());
    }
    let command_record = CommandRecord {
        argv: argv.to_vec(),
        cwd: root.display().to_string(),
        env: env_map.clone(),
    };

    if argv.is_empty() {
        return ExecutionRecord {
            id: id.to_string(),
            title: title.to_string(),
            applicable: false,
            status: "skipped".to_string(),
            elapsed_ms: 0,
            command: Some(command_record),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("empty command".to_string()),
            note: Some("no command provided".to_string()),
        };
    }

    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]).current_dir(root);
    for (k, v) in env {
        command.env(k, v);
    }

    match command.output() {
        Ok(output) => {
            let status_ok = output.status.success();
            ExecutionRecord {
                id: id.to_string(),
                title: title.to_string(),
                applicable: true,
                status: if status_ok { "ok" } else { "failed" }.to_string(),
                elapsed_ms: started.elapsed().as_millis(),
                command: Some(command_record),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
                error: if status_ok {
                    None
                } else {
                    Some("command exited with non-zero status".to_string())
                },
                note: None,
            }
        }
        Err(err) => ExecutionRecord {
            id: id.to_string(),
            title: title.to_string(),
            applicable: true,
            status: "failed".to_string(),
            elapsed_ms: started.elapsed().as_millis(),
            command: Some(command_record),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("failed to spawn command: {err}")),
            note: None,
        },
    }
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let serialized = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{serialized}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_text_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn collect_harness_artifacts(
    root: &Path,
    report_path: &Path,
    markdown_path: &Path,
) -> HarnessArtifacts {
    HarnessArtifacts {
        json_report: report_path.display().to_string(),
        markdown_summary: markdown_path.display().to_string(),
        tasklist_general: root.join("TASKLIST-GERAL.md").display().to_string(),
        capability_catalog: root
            .join("contracts/generated/capability-catalog.v1.json")
            .display()
            .to_string(),
        capability_cookbook: root
            .join("contracts/generated/cookbook.capabilities.v1.md")
            .display()
            .to_string(),
        smoke_script: root.join("scripts/smoke.sh").display().to_string(),
        contracts_script: root
            .join("scripts/validate-contracts.sh")
            .display()
            .to_string(),
        severe_suite_script: root
            .join("scripts/integration-severe.sh")
            .display()
            .to_string(),
        logic_linear_meta: root
            .join("logic.logline.world/.code247/linear-meta.json")
            .display()
            .to_string(),
        code247_linear_meta: root
            .join("code247.logline.world/.code247/linear-meta.json")
            .display()
            .to_string(),
        root_code247_dir: root.join(".code247").display().to_string(),
        code247_evidence_dir: root
            .join("code247.logline.world/evidence")
            .display()
            .to_string(),
        code247_sqlite_db: root
            .join("code247.logline.world/dual_agents.db")
            .display()
            .to_string(),
    }
}

fn render_markdown_summary(report: &HarnessReport) -> String {
    let ok = report.summary["ok"].as_bool().unwrap_or(false);
    let step_failures = report.summary["step_failures"].as_u64().unwrap_or(0);
    let scenario_failures = report.summary["scenario_failures"].as_u64().unwrap_or(0);
    let scenario_skipped = report.summary["scenario_skipped"].as_u64().unwrap_or(0);

    let mut sections = vec![
        "# Operations Verify Summary".to_string(),
        String::new(),
        format!("- request_id: `{}`", report.request_id),
        format!("- generated_at: `{}`", report.generated_at),
        format!("- root: `{}`", report.root),
        format!("- pipeline: `{}`", report.pipeline),
        format!("- ok: `{}`", ok),
        format!("- step_failures: `{}`", step_failures),
        format!("- scenario_failures: `{}`", scenario_failures),
        format!("- scenario_skipped: `{}`", scenario_skipped),
        String::new(),
        "## Artifact Links".to_string(),
        format!("- json_report: `{}`", report.artifacts.json_report),
        format!(
            "- markdown_summary: `{}`",
            report.artifacts.markdown_summary
        ),
        format!(
            "- tasklist_general: `{}`",
            report.artifacts.tasklist_general
        ),
        format!(
            "- capability_catalog: `{}`",
            report.artifacts.capability_catalog
        ),
        format!(
            "- capability_cookbook: `{}`",
            report.artifacts.capability_cookbook
        ),
        String::new(),
        "## Traceability Paths".to_string(),
        format!(
            "- logic_linear_meta: `{}`",
            report.artifacts.logic_linear_meta
        ),
        format!(
            "- code247_linear_meta: `{}`",
            report.artifacts.code247_linear_meta
        ),
        format!(
            "- root_code247_dir: `{}`",
            report.artifacts.root_code247_dir
        ),
        format!(
            "- code247_evidence_dir: `{}`",
            report.artifacts.code247_evidence_dir
        ),
        format!(
            "- code247_sqlite_db: `{}`",
            report.artifacts.code247_sqlite_db
        ),
        String::new(),
        "## Execution Steps".to_string(),
    ];

    for step in &report.steps {
        sections.push(format!(
            "- {} `{}` elapsed={}ms exit_code={}",
            step.id,
            step.status,
            step.elapsed_ms,
            step.exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string())
        ));
    }

    sections.push(String::new());
    sections.push("## Severe Scenarios".to_string());
    for scenario in &report.scenarios {
        sections.push(format!(
            "- {} `{}` applicable=`{}` elapsed={}ms",
            scenario.id, scenario.status, scenario.applicable, scenario.elapsed_ms
        ));
    }

    sections.push(String::new());
    sections.push("## Round-Trip by Intention".to_string());
    sections.push(format!(
        "- intentions_total: `{}`",
        report.round_trip["intentions_total"].as_u64().unwrap_or(0)
    ));
    sections.push(format!(
        "- linked_total: `{}`",
        report.round_trip["linked_total"].as_u64().unwrap_or(0)
    ));
    sections.push(format!(
        "- synced_total: `{}`",
        report.round_trip["synced_total"].as_u64().unwrap_or(0)
    ));
    sections.push(format!(
        "- sync_errors_total: `{}`",
        report.round_trip["sync_errors_total"].as_u64().unwrap_or(0)
    ));
    if let Some(intentions) = report
        .round_trip
        .get("intentions")
        .and_then(serde_json::Value::as_array)
    {
        for row in intentions {
            let id = row
                .get("intention_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let status = row
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            sections.push(format!("- `{id}` -> `{status}`"));
        }
    }

    let failed_steps: Vec<&ExecutionRecord> = report
        .steps
        .iter()
        .filter(|record| record.status == "failed")
        .collect();
    let failed_scenarios: Vec<&ExecutionRecord> = report
        .scenarios
        .iter()
        .filter(|record| record.status == "failed")
        .collect();

    if !failed_steps.is_empty() || !failed_scenarios.is_empty() {
        sections.push(String::new());
        sections.push("## Failures".to_string());
        for record in failed_steps.into_iter().chain(failed_scenarios.into_iter()) {
            sections.push(format!(
                "- {}: {}",
                record.id,
                record
                    .error
                    .as_deref()
                    .unwrap_or("command exited with non-zero status")
            ));
        }
    }

    sections.push(String::new());
    sections.join("\n")
}

fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = root {
        return canonical_root(path);
    }

    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    for candidate in [cwd.clone(), cwd.join(".."), cwd.join("../..")] {
        if candidate.join("TASKLIST-GERAL.md").exists() && candidate.join("scripts").exists() {
            return canonical_root(candidate);
        }
    }

    bail!("failed to resolve integration root. Pass --root <path>.");
}

fn canonical_root(root: PathBuf) -> Result<PathBuf> {
    let canonical = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    if !canonical.join("TASKLIST-GERAL.md").exists() {
        bail!(
            "invalid root {} (missing TASKLIST-GERAL.md)",
            canonical.display()
        );
    }
    Ok(canonical)
}

fn collect_round_trip_snapshot(root: &Path) -> Value {
    let manifest_path = root
        .join("logic.logline.world")
        .join(".code247")
        .join("manifest.intentions.json");
    let code247_meta_path = root
        .join("code247.logline.world")
        .join(".code247")
        .join("linear-meta.json");
    let logic_meta_path = root
        .join("logic.logline.world")
        .join(".code247")
        .join("linear-meta.json");

    let manifest = load_json_file(&manifest_path);
    let code247_meta = load_json_file(&code247_meta_path);
    let logic_meta = load_json_file(&logic_meta_path);

    let manifest_ids = manifest
        .value
        .as_ref()
        .map(|value| collect_ids(value, "/intentions", "id"))
        .unwrap_or_default();
    let linear_ids = merge_ids(&[
        (&code247_meta, "/linear/intentions", "id"),
        (&logic_meta, "/linear/intentions", "id"),
    ]);
    let synced_ids = merge_ids(&[
        (&code247_meta, "/sync/synced", "intention_id"),
        (&logic_meta, "/sync/synced", "intention_id"),
    ]);
    let sync_error_ids = merge_ids(&[
        (&code247_meta, "/sync/errors", "intention_id"),
        (&logic_meta, "/sync/errors", "intention_id"),
    ]);

    let intentions = manifest_ids
        .iter()
        .map(|id| {
            let linked = linear_ids.contains(id);
            let synced = synced_ids.contains(id);
            let sync_error = sync_error_ids.contains(id);
            let status = if synced {
                "complete"
            } else if sync_error {
                "sync_error"
            } else if linked {
                "linked"
            } else {
                "manifest_only"
            };
            json!({
                "intention_id": id,
                "linked_to_linear": linked,
                "sync_success": synced,
                "sync_error": sync_error,
                "status": status,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "manifest": json_load_snapshot(&manifest_path, &manifest),
        "code247_linear_meta": json_load_snapshot(&code247_meta_path, &code247_meta),
        "logic_linear_meta": json_load_snapshot(&logic_meta_path, &logic_meta),
        "intentions_total": manifest_ids.len(),
        "linked_total": manifest_ids.iter().filter(|id| linear_ids.contains(*id)).count(),
        "synced_total": manifest_ids.iter().filter(|id| synced_ids.contains(*id)).count(),
        "sync_errors_total": manifest_ids.iter().filter(|id| sync_error_ids.contains(*id)).count(),
        "intentions": intentions,
    })
}

fn merge_ids(sources: &[(&JsonLoadResult, &str, &str)]) -> BTreeSet<String> {
    let mut merged = BTreeSet::new();
    for (source, pointer, field) in sources {
        if let Some(value) = source.value.as_ref() {
            for id in collect_ids(value, pointer, field) {
                merged.insert(id);
            }
        }
    }
    merged
}

fn collect_ids(value: &Value, pointer: &str, field: &str) -> BTreeSet<String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get(field))
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn load_json_file(path: &Path) -> JsonLoadResult {
    if !path.exists() {
        return JsonLoadResult {
            exists: false,
            parse_ok: false,
            value: None,
            error: None,
        };
    }

    match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => JsonLoadResult {
                exists: true,
                parse_ok: true,
                value: Some(value),
                error: None,
            },
            Err(err) => JsonLoadResult {
                exists: true,
                parse_ok: false,
                value: None,
                error: Some(err.to_string()),
            },
        },
        Err(err) => JsonLoadResult {
            exists: true,
            parse_ok: false,
            value: None,
            error: Some(err.to_string()),
        },
    }
}

fn json_load_snapshot(path: &Path, load: &JsonLoadResult) -> Value {
    json!({
        "path": path.display().to_string(),
        "exists": load.exists,
        "parse_ok": load.parse_ok,
        "error": load.error,
    })
}
