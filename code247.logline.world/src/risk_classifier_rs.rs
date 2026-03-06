use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum MergeMode {
    Light,
    Substantial,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskAssessment {
    pub score: u8,
    pub merge_mode: MergeMode,
    pub diff_lines: usize,
    pub changed_files: usize,
    pub changed_modules: usize,
    pub docs_only: bool,
    pub tests_touched: bool,
    pub sensitive_paths: Vec<String>,
    pub reason_codes: Vec<String>,
}

#[derive(Default)]
pub struct RiskClassifier;

impl RiskClassifier {
    pub fn classify(files: &[String], diff_lines: usize) -> RiskAssessment {
        let changed_modules = unique_modules(files).len();
        let sensitive_paths = collect_sensitive_paths(files);
        let has_contract_change = files.iter().any(|path| is_contract_path(path));
        let has_perf_or_concurrency_change =
            files.iter().any(|path| is_perf_or_concurrency_path(path));
        let docs_only = files.iter().all(|path| is_docs_or_comment_path(path));
        let tests_touched = files.iter().any(|path| is_test_path(path));

        let mut score: u8 = 0;
        let mut reason_codes = Vec::new();

        if !sensitive_paths.is_empty() {
            score = score.saturating_add(3);
            reason_codes.push("touches_sensitive_paths".to_string());
        }

        if has_contract_change {
            score = score.saturating_add(2);
            reason_codes.push("public_contract_changed".to_string());
        }

        if diff_lines > 200 || changed_modules > 3 {
            score = score.saturating_add(2);
            if diff_lines > 200 {
                reason_codes.push("diff_large".to_string());
            }
            if changed_modules > 3 {
                reason_codes.push("modules_many".to_string());
            }
        }

        if has_perf_or_concurrency_change {
            score = score.saturating_add(1);
            reason_codes.push("perf_or_concurrency_touched".to_string());
        }

        if !docs_only && !tests_touched {
            score = score.saturating_add(1);
            reason_codes.push("tests_missing".to_string());
        }

        let merge_mode = if !sensitive_paths.is_empty() || score >= 3 {
            MergeMode::Substantial
        } else {
            MergeMode::Light
        };

        RiskAssessment {
            score,
            merge_mode,
            diff_lines,
            changed_files: files.len(),
            changed_modules,
            docs_only,
            tests_touched,
            sensitive_paths,
            reason_codes,
        }
    }
}

fn unique_modules(files: &[String]) -> BTreeSet<String> {
    files
        .iter()
        .map(|path| {
            path.split('/')
                .next()
                .filter(|segment| !segment.is_empty())
                .unwrap_or(path.as_str())
                .to_string()
        })
        .collect()
}

fn collect_sensitive_paths(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|path| is_sensitive_path(path))
        .cloned()
        .collect()
}

fn is_sensitive_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.starts_with(".github/workflows/")
        || normalized.starts_with("auth/")
        || normalized.contains("/auth/")
        || normalized.starts_with("billing/")
        || normalized.contains("/billing/")
        || normalized.starts_with("permissions/")
        || normalized.contains("/permissions/")
        || normalized.starts_with("migrations/")
        || normalized.contains("/migrations/")
        || normalized.starts_with("infra/")
        || normalized.contains("/infra/")
}

fn is_contract_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.starts_with("contracts/")
        || normalized.starts_with("schemas/")
        || normalized.contains("/schemas/")
        || normalized.contains("openapi")
        || normalized.ends_with(".schema.json")
        || normalized.contains("events.registry")
}

fn is_perf_or_concurrency_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.contains("cache")
        || normalized.contains("concurr")
        || normalized.contains("tokio")
        || normalized.contains("perf")
}

fn is_docs_or_comment_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.ends_with(".md")
        || normalized.ends_with(".txt")
        || normalized.starts_with("docs/")
        || normalized.contains("/docs/")
}

fn is_test_path(path: &str) -> bool {
    let normalized = path.to_ascii_lowercase();
    normalized.contains("/tests/")
        || normalized.starts_with("tests/")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with(".test.ts")
        || normalized.ends_with(".spec.ts")
}

#[cfg(test)]
mod tests {
    use super::{MergeMode, RiskClassifier};

    #[test]
    fn marks_sensitive_change_as_substantial() {
        let files = vec![
            "src/auth/session.rs".to_string(),
            "src/api.rs".to_string(),
            "src/api_test.rs".to_string(),
        ];
        let assessment = RiskClassifier::classify(&files, 80);
        assert_eq!(assessment.merge_mode, MergeMode::Substantial);
        assert!(assessment
            .reason_codes
            .contains(&"touches_sensitive_paths".to_string()));
    }

    #[test]
    fn marks_docs_only_as_light() {
        let files = vec!["docs/readme.md".to_string(), "CHANGELOG.md".to_string()];
        let assessment = RiskClassifier::classify(&files, 30);
        assert_eq!(assessment.merge_mode, MergeMode::Light);
        assert_eq!(assessment.score, 0);
        assert!(assessment.docs_only);
    }

    #[test]
    fn marks_large_diff_without_tests_as_substantial() {
        let files = vec![
            "src/core/a.rs".to_string(),
            "src/core/b.rs".to_string(),
            "src/core/c.rs".to_string(),
            "src/core/d.rs".to_string(),
        ];
        let assessment = RiskClassifier::classify(&files, 450);
        assert_eq!(assessment.merge_mode, MergeMode::Substantial);
        assert!(assessment.score >= 3);
        assert!(assessment.reason_codes.contains(&"diff_large".to_string()));
    }
}
