use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::{
    adapters_rs::{LinearIssue, ReviewOutput},
    persistence_rs::Job,
    risk_classifier_rs::{MergeMode, RiskAssessment},
};

#[derive(Clone)]
pub struct PrCreator {
    github_token: String,
    github_repo: String,
    base_branch: String,
    auto_merge_enabled: bool,
    auto_merge_timeout: Duration,
    auto_merge_poll: Duration,
    http: Client,
}

impl PrCreator {
    pub fn new(
        github_token: String,
        github_repo: String,
        base_branch: String,
        auto_merge_enabled: bool,
        auto_merge_timeout_seconds: u64,
        auto_merge_poll_seconds: u64,
    ) -> Self {
        Self {
            github_token,
            github_repo,
            base_branch,
            auto_merge_enabled,
            auto_merge_timeout: Duration::from_secs(auto_merge_timeout_seconds.max(60)),
            auto_merge_poll: Duration::from_secs(auto_merge_poll_seconds.max(5)),
            http: Client::new(),
        }
    }

    pub fn repo_slug(&self) -> &str {
        &self.github_repo
    }

    pub async fn create(
        &self,
        job: &Job,
        issue: &LinearIssue,
        review: &ReviewOutput,
        branch: &str,
        files: &[String],
        risk: &RiskAssessment,
    ) -> Result<(u64, String)> {
        let title = format!("feat({}): {}", issue.identifier, issue.title);
        let merge_mode = match risk.merge_mode {
            MergeMode::Light => "light",
            MergeMode::Substantial => "substantial",
        };
        let risk_reasons = if risk.reason_codes.is_empty() {
            "- none".to_string()
        } else {
            risk.reason_codes
                .iter()
                .map(|code| format!("- `{code}`"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let sensitive_paths = if risk.sensitive_paths.is_empty() {
            "- none".to_string()
        } else {
            risk.sensitive_paths
                .iter()
                .map(|path| format!("- `{path}`"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let body = format!(
            "## 🤖 job: {}\n\n**Issue**: {} — {}\n\n## Merge Policy\n- mode: `{}`\n- risk_score: `{}`\n- diff_lines: `{}`\n- changed_files: `{}`\n- changed_modules: `{}`\n- tests_touched: `{}`\n- docs_only: `{}`\n\n### Risk Reasons\n{}\n\n### Sensitive Paths\n{}\n\n## Review\n{}\n\n## Files\n{}",
            job.id,
            issue.identifier,
            issue.title,
            merge_mode,
            risk.score,
            risk.diff_lines,
            risk.changed_files,
            risk.changed_modules,
            risk.tests_touched,
            risk.docs_only,
            risk_reasons,
            sensitive_paths,
            review.summary,
            files
                .iter()
                .map(|f| format!("- `{f}`"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let url = format!("https://api.github.com/repos/{}/pulls", self.github_repo);
        let resp: GithubPr = self
            .http
            .post(url)
            .bearer_auth(&self.github_token)
            .header("User-Agent", "code247-agent")
            .json(&json!({
                "title": title,
                "head": branch,
                "base": self.base_branch,
                "body": body,
                "draft": false,
            }))
            .send()
            .await
            .context("falha ao chamar GitHub pulls API")?
            .error_for_status()
            .context("GitHub retornou erro ao criar PR")?
            .json()
            .await
            .context("resposta GitHub inválida")?;

        Ok((resp.number, resp.html_url))
    }

    pub async fn auto_merge_when_ready(
        &self,
        pr_number: u64,
        risk: &RiskAssessment,
        cloud_approved: bool,
    ) -> Result<AutoMergeOutcome> {
        if !self.auto_merge_enabled {
            return Ok(AutoMergeOutcome {
                attempted: false,
                merged: false,
                reason: "auto-merge disabled by configuration".to_string(),
                merge_commit_sha: None,
            });
        }
        if risk.merge_mode == MergeMode::Substantial && !cloud_approved {
            return Ok(AutoMergeOutcome {
                attempted: false,
                merged: false,
                reason: "risk policy: substantial change without cloud approval".to_string(),
                merge_commit_sha: None,
            });
        }

        let deadline = Instant::now() + self.auto_merge_timeout;
        loop {
            let pr = self.get_pull_request(pr_number).await?;
            if pr.merged {
                return Ok(AutoMergeOutcome {
                    attempted: true,
                    merged: true,
                    reason: "already merged".to_string(),
                    merge_commit_sha: None,
                });
            }

            let checks_ok = self.commit_checks_green(&pr.head.sha).await?;
            let mergeable_clean =
                pr.mergeable.unwrap_or(false) && pr.mergeable_state.eq_ignore_ascii_case("clean");

            if checks_ok && mergeable_clean {
                let merge_commit_sha = self.merge_pull_request(pr_number).await?;
                return Ok(AutoMergeOutcome {
                    attempted: true,
                    merged: true,
                    reason: "checks green and mergeable=clean".to_string(),
                    merge_commit_sha,
                });
            }

            if Instant::now() >= deadline {
                let checks_state = self.commit_checks_state(&pr.head.sha).await?;
                return Ok(AutoMergeOutcome {
                    attempted: true,
                    merged: false,
                    reason: format!(
                        "timeout waiting readiness (mergeable={:?}, mergeable_state={}, checks={checks_state})",
                        pr.mergeable, pr.mergeable_state
                    ),
                    merge_commit_sha: None,
                });
            }

            sleep(self.auto_merge_poll).await;
        }
    }

    async fn get_pull_request(&self, pr_number: u64) -> Result<GithubPullDetails> {
        let url = format!(
            "https://api.github.com/repos/{}/pulls/{}",
            self.github_repo, pr_number
        );
        self.http
            .get(url)
            .bearer_auth(&self.github_token)
            .header("User-Agent", "code247-agent")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("falha ao buscar detalhes do PR no GitHub")?
            .error_for_status()
            .context("GitHub retornou erro ao consultar PR")?
            .json::<GithubPullDetails>()
            .await
            .context("resposta inválida ao consultar PR")
    }

    async fn commit_checks_green(&self, sha: &str) -> Result<bool> {
        Ok(self
            .commit_checks_state(sha)
            .await?
            .eq_ignore_ascii_case("success"))
    }

    async fn commit_checks_state(&self, sha: &str) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{}/commits/{}/status",
            self.github_repo, sha
        );
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.github_token)
            .header("User-Agent", "code247-agent")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .context("falha ao buscar status checks do commit")?
            .error_for_status()
            .context("GitHub retornou erro ao consultar status checks")?
            .json::<GithubCombinedStatus>()
            .await
            .context("resposta inválida ao consultar status checks")?;
        Ok(response.state)
    }

    async fn merge_pull_request(&self, pr_number: u64) -> Result<Option<String>> {
        let url = format!(
            "https://api.github.com/repos/{}/pulls/{}/merge",
            self.github_repo, pr_number
        );
        let response = self
            .http
            .put(url)
            .bearer_auth(&self.github_token)
            .header("User-Agent", "code247-agent")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&json!({
                "merge_method": "squash",
            }))
            .send()
            .await
            .context("falha ao chamar merge do PR no GitHub")?
            .error_for_status()
            .context("GitHub recusou merge do PR")?;
        let merge_response = response
            .json::<GithubMergeResponse>()
            .await
            .context("resposta inválida do merge GitHub")?;
        Ok(merge_response.sha)
    }
}

#[derive(Deserialize)]
struct GithubPr {
    number: u64,
    html_url: String,
}

#[derive(Deserialize)]
struct GithubMergeResponse {
    sha: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AutoMergeOutcome {
    pub attempted: bool,
    pub merged: bool,
    pub reason: String,
    pub merge_commit_sha: Option<String>,
}

#[derive(Deserialize)]
struct GithubPullDetails {
    mergeable: Option<bool>,
    mergeable_state: String,
    merged: bool,
    head: GithubPullHead,
}

#[derive(Deserialize)]
struct GithubPullHead {
    sha: String,
}

#[derive(Deserialize)]
struct GithubCombinedStatus {
    state: String,
}
