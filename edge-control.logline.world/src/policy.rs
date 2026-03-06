use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use crate::models::{AppliedRule, GateDecisionV1, GateDerivedFrom};
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct PolicySet {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub defaults: PolicyDefaults,
    #[serde(default)]
    pub global_rules: Vec<Rule>,
    #[serde(default)]
    pub domains: HashMap<String, DomainPolicy>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PolicyDefaults {
    #[serde(default = "default_true")]
    pub fail_closed: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DomainPolicy {
    #[serde(default)]
    pub required_input_fields: Vec<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub id: String,
    #[serde(default)]
    pub priority: i32,
    pub decision: String,
    #[serde(default)]
    pub when: RuleWhen,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RuleWhen {
    #[serde(default)]
    pub reason_codes_any: Vec<String>,
    #[serde(default)]
    pub reason_codes_none: Vec<String>,
    #[serde(default)]
    pub missing_any_fields: Vec<String>,
    #[serde(default)]
    pub signal_eq: Option<String>,
    #[serde(default)]
    pub confidence_gte: Option<f64>,
    #[serde(default)]
    pub confidence_lt: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PolicyInput {
    pub signal: String,
    pub confidence: f64,
    pub reason_codes: Vec<String>,
    pub fields_present: HashSet<String>,
}

impl PolicySet {
    pub fn load(path: &str) -> Result<Self> {
        let bytes =
            fs::read(path).with_context(|| format!("failed to read policy-set file: {path}"))?;
        let policy: PolicySet = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse policy-set json: {path}"))?;

        Ok(policy)
    }

    pub fn evaluate(
        &self,
        domain: &str,
        input: &PolicyInput,
        trace_id: &str,
        opinion_event_id: &str,
    ) -> GateDecisionV1 {
        let mut matched: Vec<(Rule, &'static str)> = Vec::new();

        for rule in &self.global_rules {
            if rule_matches(rule, input) {
                matched.push((rule.clone(), "global"));
            }
        }

        let domain_policy = self.domains.get(domain);

        if let Some(dp) = domain_policy {
            for rule in &dp.rules {
                if rule_matches(rule, input) {
                    matched.push((rule.clone(), "domain"));
                }
            }
        }

        matched.sort_by(|a, b| {
            b.0.priority
                .cmp(&a.0.priority)
                .then_with(|| a.0.id.cmp(&b.0.id))
        });

        let selected = matched
            .first()
            .map(|(rule, _)| rule.decision.clone())
            .unwrap_or_else(|| {
                if self.defaults.fail_closed {
                    "NO".to_string()
                } else {
                    "HUMAN".to_string()
                }
            });

        let mut applied_rules: Vec<AppliedRule> = matched
            .iter()
            .take(10)
            .map(|(rule, scope)| AppliedRule {
                id: rule.id.clone(),
                result: "pass".to_string(),
                note: Some(format!("matched {scope} rule; priority={}", rule.priority)),
            })
            .collect();

        if applied_rules.is_empty() {
            applied_rules.push(AppliedRule {
                id: "DEFAULT-FAIL-CLOSED".into(),
                result: "pass".into(),
                note: Some("no matching rule; default decision applied".into()),
            });
        }

        GateDecisionV1 {
            version: "gate-decision.v1",
            decision: selected,
            policy: format!(
                "{}/{}",
                if self.version.is_empty() {
                    "policy-set.unknown"
                } else {
                    &self.version
                },
                domain
            ),
            applied_rules,
            derived_from: GateDerivedFrom {
                opinion_event_id: opinion_event_id.to_string(),
                trace_id: trace_id.to_string(),
            },
        }
    }
}

fn rule_matches(rule: &Rule, input: &PolicyInput) -> bool {
    let reasons: HashSet<&str> = input.reason_codes.iter().map(String::as_str).collect();

    if !rule.when.reason_codes_any.is_empty()
        && !rule
            .when
            .reason_codes_any
            .iter()
            .any(|code| reasons.contains(code.as_str()))
    {
        return false;
    }

    if !rule.when.reason_codes_none.is_empty()
        && rule
            .when
            .reason_codes_none
            .iter()
            .any(|code| reasons.contains(code.as_str()))
    {
        return false;
    }

    if !rule.when.missing_any_fields.is_empty()
        && !rule
            .when
            .missing_any_fields
            .iter()
            .any(|field| !input.fields_present.contains(field.as_str()))
    {
        return false;
    }

    if let Some(signal_eq) = &rule.when.signal_eq {
        if &input.signal != signal_eq {
            return false;
        }
    }

    if let Some(confidence_gte) = rule.when.confidence_gte {
        if input.confidence < confidence_gte {
            return false;
        }
    }

    if let Some(confidence_lt) = rule.when.confidence_lt {
        if input.confidence >= confidence_lt {
            return false;
        }
    }

    true
}

pub fn resolve_policy_set_path(config_path: &str) -> String {
    if Path::new(config_path).is_absolute() {
        return config_path.to_string();
    }

    config_path.to_string()
}
