use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ErrorResponseV1 {
    pub request_id: String,
    pub output_schema: &'static str,
    pub error: ErrorDetailV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetailV1 {
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
    pub message: String,
}

impl ErrorResponseV1 {
    const OUTPUT_SCHEMA: &'static str =
        "https://logline.world/schemas/error-envelope.v1.schema.json";

    pub fn new(
        request_id: Option<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let request_id = request_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let code = code.into();
        Self {
            trace_id: Some(request_id.clone()),
            request_id: request_id.clone(),
            output_schema: Self::OUTPUT_SCHEMA,
            error: ErrorDetailV1 {
                error_type: code.clone(),
                code,
                message: message.into(),
            },
        }
    }

    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        if let Some(request_id) = request_id {
            self.request_id = request_id.clone();
            self.trace_id = Some(request_id);
        }
        self
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub request_id: String,
    pub output_schema: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct IntentionDraftRequestV1 {
    pub version: String,
    pub intent_text: String,
    pub context: IntentionDraftContext,
}

#[derive(Debug, Deserialize)]
pub struct IntentionDraftContext {
    pub repo: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub source: Option<IntentionSourceRef>,
    #[serde(default)]
    pub constraints_hint: Option<ConstraintsHint>,
}

#[derive(Debug, Deserialize)]
pub struct IntentionSourceRef {
    pub kind: String,
    #[serde(default)]
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConstraintsHint {
    #[serde(default)]
    pub risk_tier: Option<String>,
    #[serde(default)]
    pub max_files_changed: Option<i32>,
    #[serde(default)]
    pub max_diff_lines: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct PrRiskRequestV1 {
    pub version: String,
    pub pr_id: String,
    pub repo: String,
    pub base_branch: String,
    pub head_branch: String,
    pub diff_stats: DiffStats,
    pub touched_paths: Vec<String>,
    #[serde(default)]
    pub patch_snippets: Vec<String>,
    #[serde(default)]
    pub intent_ref: Option<IntentRef>,
}

#[derive(Debug, Deserialize)]
pub struct DiffStats {
    pub files_changed: i32,
    pub lines_added: i32,
    pub lines_deleted: i32,
}

#[derive(Debug, Deserialize)]
pub struct IntentRef {
    #[serde(default)]
    pub intention_id: Option<String>,
    #[serde(default)]
    pub issue_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FuelDiffRouteRequestV1 {
    pub version: String,
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
    pub drift_score: f64,
    pub metric_deltas: Value,
    pub flags: Vec<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OpinionSignalV1 {
    pub request_id: String,
    pub output_schema: &'static str,
    pub version: &'static str,
    pub signal: String,
    pub confidence: f64,
    pub reason_codes: Vec<String>,
    pub evidence: Vec<EvidenceItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<OpinionProposal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GateDecisionV1 {
    pub version: &'static str,
    pub decision: String,
    pub policy: String,
    pub applied_rules: Vec<AppliedRule>,
    pub derived_from: GateDerivedFrom,
}

#[derive(Debug, Serialize)]
pub struct AppliedRule {
    pub id: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GateDerivedFrom {
    pub opinion_event_id: String,
    pub trace_id: String,
}

#[derive(Debug, Serialize)]
pub struct EvidenceItem {
    pub kind: String,
    pub r#ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct OpinionProposal {
    pub next: Vec<String>,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct DraftIntentionV1 {
    pub request_id: String,
    pub output_schema: &'static str,
    pub version: &'static str,
    pub draft_intention_id: String,
    pub created_at: String,
    pub author: DraftAuthor,
    pub source: DraftSource,
    pub ast: DraftAst,
    pub diagnostics: Vec<DraftDiagnostic>,
    pub questions: Vec<DraftQuestion>,
}

#[derive(Debug, Serialize)]
pub struct DraftAuthor {
    pub kind: &'static str,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct DraftSource {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DraftAst {
    pub version: &'static str,
    pub title: String,
    pub goal: String,
    pub scope: DraftScope,
    pub acceptance_criteria: Vec<String>,
    pub constraints: DraftConstraints,
    pub rollback: RollbackPlanV1,
}

#[derive(Debug, Serialize)]
pub struct DraftScope {
    pub r#in: Vec<String>,
    pub out: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DraftConstraints {
    pub risk_tier: String,
    pub max_files_changed: i32,
    pub max_diff_lines: i32,
    pub rollback_required: bool,
    pub requires_human_review: bool,
}

#[derive(Debug, Serialize)]
pub struct DraftDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DraftQuestion {
    pub id: String,
    pub question: String,
    pub field_target: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RollbackPlanV1 {
    pub version: &'static str,
    pub strategy: String,
    pub steps: Vec<String>,
    pub verification: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<RollbackTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_lock_minutes: Option<i32>,
    pub requires_two_person_confirm: bool,
}

#[derive(Debug, Serialize)]
pub struct RollbackTarget {
    pub kind: String,
    pub r#ref: String,
}
