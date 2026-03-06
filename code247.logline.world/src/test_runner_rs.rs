use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;

#[derive(Clone)]
pub struct TestRunner {
    repo_root: PathBuf,
    flaky_reruns: u8,
    red_main_enforced: bool,
    red_main_flag_path: PathBuf,
    runner_allowlist_enabled: bool,
    runner_allowlist_manifest_path: PathBuf,
}

#[derive(Serialize, Clone)]
pub struct ValidationResult {
    pub passed: bool,
    pub cargo_output: String,
    pub npm_output: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub flaky_recovered: bool,
    pub red_main_blocked: bool,
    pub cargo_attempts: u8,
    pub npm_attempts: Option<u8>,
    pub runner_allowlist_enforced: bool,
    pub runner_allowlist_source: Option<String>,
}

impl TestRunner {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        flaky_reruns: u8,
        red_main_enforced: bool,
        red_main_flag_path: impl Into<PathBuf>,
        runner_allowlist_enabled: bool,
        runner_allowlist_manifest_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            flaky_reruns,
            red_main_enforced,
            red_main_flag_path: red_main_flag_path.into(),
            runner_allowlist_enabled,
            runner_allowlist_manifest_path: runner_allowlist_manifest_path.into(),
        }
    }

    pub async fn validate(&self) -> Result<ValidationResult> {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let runner_allowlist = if self.runner_allowlist_enabled {
            Some(
                load_runner_allowlist(&self.repo_root, &self.runner_allowlist_manifest_path)
                    .map_err(|err| anyhow!("runner allowlist inválida: {err}"))?,
            )
        } else {
            warnings.push(
                "runner_allowlist disabled: execução sem policy explícita de comandos".to_string(),
            );
            None
        };

        if self.red_main_enforced && self.red_main_flag_path.exists() {
            errors.push(format!(
                "red-main ativo: flag encontrada em '{}'; novas features pausadas até restaurar main verde",
                self.red_main_flag_path.display()
            ));
            return Ok(ValidationResult {
                passed: false,
                cargo_output: String::new(),
                npm_output: None,
                warnings,
                errors,
                flaky_recovered: false,
                red_main_blocked: true,
                cargo_attempts: 0,
                npm_attempts: None,
                runner_allowlist_enforced: self.runner_allowlist_enabled,
                runner_allowlist_source: runner_allowlist
                    .as_ref()
                    .map(|value| value.source.display().to_string()),
            });
        }

        let cargo = run_cmd_with_retry(
            &self.repo_root,
            "cargo",
            &["check"],
            self.flaky_reruns,
            runner_allowlist.as_ref(),
        )
        .await?;
        if !cargo.ok {
            errors.push(format!(
                "cargo check falhou após {} tentativa(s)",
                cargo.attempts
            ));
        }
        if cargo.recovered_after_retry {
            warnings
                .push("flaky_detected: cargo check recuperou após re-run automático".to_string());
        }

        let npm = run_cmd_with_retry(
            &self.repo_root,
            "npm",
            &["run", "typecheck"],
            self.flaky_reruns,
            runner_allowlist.as_ref(),
        )
        .await;
        if let Ok(run) = &npm {
            if !run.ok {
                errors.push(format!(
                    "npm run typecheck falhou após {} tentativa(s)",
                    run.attempts
                ));
            }
            if run.recovered_after_retry {
                warnings.push(
                    "flaky_detected: npm run typecheck recuperou após re-run automático"
                        .to_string(),
                );
            }
        } else if let Err(err) = &npm {
            if err.to_string().contains("runner allowlist deny") {
                errors.push(err.to_string());
            } else {
                warnings.push(format!("npm run typecheck ignorado: {err}"));
            }
        }

        let (npm_output, npm_attempts, npm_flaky_recovered) = match npm {
            Ok(run) => (
                Some(run.output),
                Some(run.attempts),
                run.recovered_after_retry,
            ),
            Err(_) => (None, None, false),
        };
        let flaky_recovered = cargo.recovered_after_retry || npm_flaky_recovered;

        Ok(ValidationResult {
            passed: errors.is_empty(),
            cargo_output: cargo.output,
            npm_output,
            warnings,
            errors,
            flaky_recovered,
            red_main_blocked: false,
            cargo_attempts: cargo.attempts,
            npm_attempts,
            runner_allowlist_enforced: self.runner_allowlist_enabled,
            runner_allowlist_source: runner_allowlist
                .as_ref()
                .map(|value| value.source.display().to_string()),
        })
    }
}

struct RunnerAllowlist {
    source: PathBuf,
    entries: Vec<Vec<String>>,
}

struct CommandRun {
    ok: bool,
    output: String,
    attempts: u8,
    recovered_after_retry: bool,
}

async fn run_cmd_with_retry(
    cwd: &PathBuf,
    cmd: &str,
    args: &[&str],
    flaky_reruns: u8,
    runner_allowlist: Option<&RunnerAllowlist>,
) -> Result<CommandRun> {
    ensure_command_allowed(cmd, args, runner_allowlist)?;

    let max_attempts = flaky_reruns.saturating_add(1).max(1);
    let mut output_log = String::new();

    for attempt in 1..=max_attempts {
        let output = Command::new(cmd)
            .args(args)
            .current_dir(cwd)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        output_log.push_str(&format!(
            "--- attempt {attempt}/{max_attempts}: {} {} ---\n{}\n{}\n",
            cmd,
            args.join(" "),
            stdout,
            stderr
        ));

        if output.status.success() {
            return Ok(CommandRun {
                ok: true,
                output: output_log,
                attempts: attempt,
                recovered_after_retry: attempt > 1,
            });
        }
    }

    Ok(CommandRun {
        ok: false,
        output: output_log,
        attempts: max_attempts,
        recovered_after_retry: false,
    })
}

fn ensure_command_allowed(
    cmd: &str,
    args: &[&str],
    runner_allowlist: Option<&RunnerAllowlist>,
) -> Result<()> {
    let Some(runner_allowlist) = runner_allowlist else {
        return Ok(());
    };
    let mut requested = vec![cmd.to_string()];
    requested.extend(args.iter().map(|value| (*value).to_string()));

    let allowed = runner_allowlist
        .entries
        .iter()
        .any(|entry| requested.starts_with(entry));
    if allowed {
        return Ok(());
    }

    let allowed_commands = runner_allowlist
        .entries
        .iter()
        .map(|entry| entry.join(" "))
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "runner allowlist deny: '{}' não está autorizado (allowlist: {})",
        requested.join(" "),
        allowed_commands
    );
}

fn load_runner_allowlist(repo_root: &Path, manifest_path: &Path) -> Result<RunnerAllowlist> {
    let absolute_manifest_path = if manifest_path.is_absolute() {
        manifest_path.to_path_buf()
    } else {
        repo_root.join(manifest_path)
    };
    let raw = fs::read_to_string(&absolute_manifest_path).map_err(|err| {
        anyhow!(
            "falha ao ler manifesto de allowlist '{}': {err}",
            absolute_manifest_path.display()
        )
    })?;
    let manifest: Value = serde_json::from_str(&raw).map_err(|err| {
        anyhow!(
            "manifesto de allowlist inválido em '{}': {err}",
            absolute_manifest_path.display()
        )
    })?;
    let commands = manifest
        .pointer("/gates/gate0/commands")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anyhow!(
                "manifesto '{}' sem gates.gate0.commands",
                absolute_manifest_path.display()
            )
        })?;

    let mut entries = Vec::new();
    for command in commands {
        if let Some(raw_command) = command.as_str() {
            let tokens = raw_command
                .split_whitespace()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if !tokens.is_empty() {
                entries.push(tokens);
            }
        }
    }
    if entries.is_empty() {
        bail!(
            "manifesto '{}' possui gates.gate0.commands vazio",
            absolute_manifest_path.display()
        );
    }

    Ok(RunnerAllowlist {
        source: absolute_manifest_path,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::{ensure_command_allowed, load_runner_allowlist};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nonce}"));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn loads_runner_allowlist_from_manifest() {
        let root = temp_dir("code247-runner-allowlist");
        let manifest_path = root.join("workspace.manifest.json");
        fs::write(
            &manifest_path,
            r#"{
  "gates": {
    "gate0": {
      "commands": ["cargo check", "npm run typecheck"]
    }
  }
}"#,
        )
        .expect("manifest should be written");

        let allowlist =
            load_runner_allowlist(&root, PathBuf::from("workspace.manifest.json").as_path())
                .expect("allowlist should load");
        assert_eq!(allowlist.entries.len(), 2);
        assert_eq!(allowlist.entries[0], vec!["cargo", "check"]);
        assert_eq!(allowlist.entries[1], vec!["npm", "run", "typecheck"]);

        fs::remove_dir_all(root).expect("temp dir should be removed");
    }

    #[test]
    fn blocks_command_outside_allowlist() {
        let root = temp_dir("code247-runner-deny");
        let manifest_path = root.join("workspace.manifest.json");
        fs::write(
            &manifest_path,
            r#"{
  "gates": {
    "gate0": {
      "commands": ["cargo check", "npm run typecheck"]
    }
  }
}"#,
        )
        .expect("manifest should be written");

        let allowlist =
            load_runner_allowlist(&root, PathBuf::from("workspace.manifest.json").as_path())
                .expect("allowlist should load");
        ensure_command_allowed("cargo", &["check"], Some(&allowlist))
            .expect("allowed command should pass");
        let err = ensure_command_allowed("bash", &["-lc", "echo hi"], Some(&allowlist))
            .expect_err("unknown command should fail");
        assert!(err.to_string().contains("runner allowlist deny"));

        fs::remove_dir_all(root).expect("temp dir should be removed");
    }
}
