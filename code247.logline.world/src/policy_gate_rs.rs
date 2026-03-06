use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::adapters_rs::CloudGateDecision;

#[derive(Clone, Debug)]
pub struct PrRiskPolicy {
    allowed_decisions: HashSet<String>,
    auto_merge_confidence_min: f64,
    deny_reason_codes: HashSet<String>,
    metadata: PolicyMetadata,
}

#[derive(Clone, Debug, Serialize)]
pub struct PolicyEvaluation {
    pub allowed: bool,
    pub reason: String,
    pub cloud_decision: String,
    pub confidence: f64,
    pub matched_deny_codes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PolicyMetadata {
    pub version: String,
    pub source_path: String,
    pub source_sha256: String,
}

impl PrRiskPolicy {
    pub fn load_from_path(path: &str, required: bool) -> Result<Self> {
        let policy_path = Path::new(path);
        let resolved_path = policy_path
            .canonicalize()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.to_string());
        if !policy_path.exists() {
            if required {
                anyhow::bail!("policy-set obrigatório ausente: {}", policy_path.display());
            }
            return Ok(Self::default_fail_closed(&resolved_path));
        }

        let raw = fs::read_to_string(policy_path)
            .with_context(|| format!("falha ao ler policy-set em {}", policy_path.display()))?;
        let file: PolicySetFile = serde_json::from_str(&raw)
            .with_context(|| format!("policy-set inválido em {}", policy_path.display()))?;

        Ok(Self::from_policy_file(file, &resolved_path, Some(&raw)))
    }

    pub fn metadata(&self) -> &PolicyMetadata {
        &self.metadata
    }

    pub fn evaluate_cloud_decision(&self, decision: &CloudGateDecision) -> PolicyEvaluation {
        let normalized_decision = decision.decision.to_ascii_uppercase();

        if !self.allowed_decisions.contains(&normalized_decision) {
            return PolicyEvaluation {
                allowed: false,
                reason: format!(
                    "decision '{}' não permitida pela policy",
                    normalized_decision
                ),
                cloud_decision: normalized_decision,
                confidence: decision.confidence,
                matched_deny_codes: vec![],
            };
        }

        if normalized_decision != "YES" {
            return PolicyEvaluation {
                allowed: false,
                reason: format!(
                    "decision '{}' bloqueia auto-merge substantial",
                    normalized_decision
                ),
                cloud_decision: normalized_decision,
                confidence: decision.confidence,
                matched_deny_codes: vec![],
            };
        }

        if decision.confidence < self.auto_merge_confidence_min {
            return PolicyEvaluation {
                allowed: false,
                reason: format!(
                    "confidence {:.3} abaixo do mínimo {:.3}",
                    decision.confidence, self.auto_merge_confidence_min
                ),
                cloud_decision: normalized_decision,
                confidence: decision.confidence,
                matched_deny_codes: vec![],
            };
        }

        let matched_deny_codes = decision
            .reason_codes
            .iter()
            .filter(|code| self.deny_reason_codes.contains(&code.to_ascii_lowercase()))
            .cloned()
            .collect::<Vec<_>>();
        if !matched_deny_codes.is_empty() {
            return PolicyEvaluation {
                allowed: false,
                reason: "reason_codes bloqueados pela policy".to_string(),
                cloud_decision: normalized_decision,
                confidence: decision.confidence,
                matched_deny_codes,
            };
        }

        PolicyEvaluation {
            allowed: true,
            reason: "cloud decision aprovada pela policy".to_string(),
            cloud_decision: normalized_decision,
            confidence: decision.confidence,
            matched_deny_codes: vec![],
        }
    }

    fn default_fail_closed(source_path: &str) -> Self {
        Self {
            allowed_decisions: ["YES".to_string(), "NO".to_string(), "CLOUD".to_string()]
                .into_iter()
                .collect(),
            auto_merge_confidence_min: 0.95,
            deny_reason_codes: ["secrets_suspected".to_string(), "pii_suspected".to_string()]
                .into_iter()
                .collect(),
            metadata: PolicyMetadata {
                version: "embedded-default".to_string(),
                source_path: source_path.to_string(),
                source_sha256: "embedded-default".to_string(),
            },
        }
    }

    fn from_policy_file(file: PolicySetFile, source_path: &str, raw: Option<&str>) -> Self {
        let allowed_decisions = file
            .defaults
            .and_then(|d| d.allowed_decisions)
            .unwrap_or_else(|| vec!["YES".to_string(), "NO".to_string(), "CLOUD".to_string()])
            .into_iter()
            .map(|d| d.to_ascii_uppercase())
            .collect::<HashSet<_>>();

        let auto_merge_confidence_min = file
            .domains
            .pr_risk
            .as_ref()
            .and_then(|d| d.thresholds.as_ref())
            .and_then(|t| t.auto_merge_confidence_min)
            .unwrap_or(0.9)
            .clamp(0.0, 1.0);

        let mut deny_reason_codes = HashSet::new();
        for rule in file.global_rules {
            if rule.decision.eq_ignore_ascii_case("NO") {
                for code in rule.when.reason_codes_any.unwrap_or_default() {
                    deny_reason_codes.insert(code.to_ascii_lowercase());
                }
            }
        }
        if let Some(pr_risk) = file.domains.pr_risk {
            for rule in pr_risk.rules {
                if rule.decision.eq_ignore_ascii_case("NO") {
                    for code in rule.when.reason_codes_any.unwrap_or_default() {
                        deny_reason_codes.insert(code.to_ascii_lowercase());
                    }
                }
            }
        }

        let source_sha256 = raw
            .map(sha256_hex)
            .unwrap_or_else(|| "not-computed".to_string());

        Self {
            allowed_decisions,
            auto_merge_confidence_min,
            deny_reason_codes,
            metadata: PolicyMetadata {
                version: file.version.unwrap_or_else(|| "unknown".to_string()),
                source_path: source_path.to_string(),
                source_sha256,
            },
        }
    }
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

#[derive(Debug, Deserialize)]
struct PolicySetFile {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    defaults: Option<PolicyDefaults>,
    #[serde(default)]
    global_rules: Vec<PolicyRule>,
    domains: PolicyDomains,
}

#[derive(Debug, Deserialize)]
struct PolicyDefaults {
    #[serde(default)]
    allowed_decisions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PolicyDomains {
    #[serde(default)]
    pr_risk: Option<PolicyPrRiskDomain>,
}

#[derive(Debug, Deserialize)]
struct PolicyPrRiskDomain {
    #[serde(default)]
    thresholds: Option<PrRiskThresholds>,
    #[serde(default)]
    rules: Vec<PolicyRule>,
}

#[derive(Debug, Deserialize)]
struct PrRiskThresholds {
    #[serde(default)]
    auto_merge_confidence_min: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PolicyRule {
    decision: String,
    #[serde(default)]
    when: RuleWhen,
}

#[derive(Debug, Default, Deserialize)]
struct RuleWhen {
    #[serde(default)]
    reason_codes_any: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, CloudGateDecision, PolicySetFile, PrRiskPolicy};

    fn test_policy() -> PrRiskPolicy {
        let raw = r#"{
          "version": "policy-set.v1.1",
          "defaults": { "allowed_decisions": ["YES","NO","CLOUD"] },
          "global_rules": [
            { "decision": "NO", "when": { "reason_codes_any": ["pii_suspected"] } }
          ],
          "domains": {
            "pr_risk": {
              "thresholds": { "auto_merge_confidence_min": 0.9 },
              "rules": [
                { "decision": "NO", "when": { "reason_codes_any": ["tests_missing"] } }
              ]
            }
          }
        }"#;
        let parsed: PolicySetFile = serde_json::from_str(raw).expect("policy json");
        PrRiskPolicy::from_policy_file(parsed, "test-policy.json", Some(raw))
    }

    #[test]
    fn blocks_when_confidence_below_threshold() {
        let policy = test_policy();
        let eval = policy.evaluate_cloud_decision(&CloudGateDecision {
            decision: "YES".to_string(),
            confidence: 0.5,
            reason_codes: vec!["blast_radius_low".to_string()],
            rationale: "test".to_string(),
        });
        assert!(!eval.allowed);
    }

    #[test]
    fn blocks_on_global_deny_reason_code() {
        let policy = test_policy();
        let eval = policy.evaluate_cloud_decision(&CloudGateDecision {
            decision: "YES".to_string(),
            confidence: 0.99,
            reason_codes: vec!["pii_suspected".to_string()],
            rationale: "test".to_string(),
        });
        assert!(!eval.allowed);
    }

    #[test]
    fn allows_yes_with_high_confidence_and_clean_reasons() {
        let policy = test_policy();
        let eval = policy.evaluate_cloud_decision(&CloudGateDecision {
            decision: "YES".to_string(),
            confidence: 0.97,
            reason_codes: vec!["blast_radius_low".to_string()],
            rationale: "test".to_string(),
        });
        assert!(eval.allowed);
    }

    #[test]
    fn keeps_audit_metadata() {
        let policy = test_policy();
        let md = policy.metadata();
        assert_eq!(md.version, "policy-set.v1.1");
        assert_eq!(md.source_path, "test-policy.json");
        assert_eq!(
            md.source_sha256,
            sha256_hex(
                r#"{
          "version": "policy-set.v1.1",
          "defaults": { "allowed_decisions": ["YES","NO","CLOUD"] },
          "global_rules": [
            { "decision": "NO", "when": { "reason_codes_any": ["pii_suspected"] } }
          ],
          "domains": {
            "pr_risk": {
              "thresholds": { "auto_merge_confidence_min": 0.9 },
              "rules": [
                { "decision": "NO", "when": { "reason_codes_any": ["tests_missing"] } }
              ]
            }
          }
        }"#
            )
        );
    }
}
