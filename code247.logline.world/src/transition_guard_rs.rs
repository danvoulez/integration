#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearWorkflowState {
    Ready,
    InProgress,
    ReadyForRelease,
    Done,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncTransitionEvaluation {
    pub requested_transition: bool,
    pub has_ci_evidence: bool,
    pub has_deploy_evidence: bool,
    pub target_state: Option<LinearWorkflowState>,
    pub block: Option<SyncTransitionBlock>,
}

#[derive(Debug, Clone, Copy)]
pub struct SyncTransitionBlock {
    pub code: &'static str,
    pub hard_block: bool,
}

pub fn classify_linear_workflow_state(
    state_name: &str,
    state_type: &str,
    ready_state_name: &str,
    in_progress_state_name: &str,
    ready_for_release_state_name: &str,
    done_state_type: &str,
) -> LinearWorkflowState {
    let normalized_name = state_name.trim().to_ascii_lowercase();
    let normalized_type = state_type.trim().to_ascii_lowercase();
    let normalized_ready = ready_state_name.trim().to_ascii_lowercase();
    let normalized_in_progress = in_progress_state_name.trim().to_ascii_lowercase();
    let normalized_ready_for_release = ready_for_release_state_name.trim().to_ascii_lowercase();
    let normalized_done_type = done_state_type.trim().to_ascii_lowercase();

    if !normalized_done_type.is_empty() && normalized_type == normalized_done_type {
        return LinearWorkflowState::Done;
    }
    if normalized_name == normalized_ready_for_release || normalized_name == "ready for release" {
        return LinearWorkflowState::ReadyForRelease;
    }
    if normalized_name == normalized_in_progress || normalized_name.starts_with("in progress") {
        return LinearWorkflowState::InProgress;
    }
    if normalized_name == normalized_ready || normalized_name == "ready" {
        return LinearWorkflowState::Ready;
    }
    if normalized_name == "done" || normalized_type == "completed" {
        return LinearWorkflowState::Done;
    }
    LinearWorkflowState::Unknown
}

pub fn is_linear_transition_allowed(from: LinearWorkflowState, to: LinearWorkflowState) -> bool {
    if from == to {
        return true;
    }
    matches!(
        (from, to),
        (LinearWorkflowState::Ready, LinearWorkflowState::InProgress)
            | (
                LinearWorkflowState::InProgress,
                LinearWorkflowState::ReadyForRelease
            )
            | (
                LinearWorkflowState::ReadyForRelease,
                LinearWorkflowState::Done
            )
    )
}

pub fn requested_workflow_transition(
    requested_transition: bool,
    has_ci_evidence: bool,
    has_deploy_evidence: bool,
) -> Option<LinearWorkflowState> {
    if !requested_transition || !has_ci_evidence {
        return None;
    }
    if has_deploy_evidence {
        Some(LinearWorkflowState::Done)
    } else {
        Some(LinearWorkflowState::ReadyForRelease)
    }
}

pub fn evaluate_transition(
    requested_transition: bool,
    has_ci_evidence: bool,
    has_deploy_evidence: bool,
    current_state: LinearWorkflowState,
) -> SyncTransitionEvaluation {
    let target_state =
        requested_workflow_transition(requested_transition, has_ci_evidence, has_deploy_evidence);

    let block = if requested_transition && !has_ci_evidence {
        Some(SyncTransitionBlock {
            code: "EVIDENCE_REQUIRED",
            hard_block: false,
        })
    } else if let Some(target) = target_state {
        if !is_linear_transition_allowed(current_state, target) {
            Some(SyncTransitionBlock {
                code: "INVALID_STATE_TRANSITION",
                hard_block: true,
            })
        } else {
            None
        }
    } else {
        None
    };

    SyncTransitionEvaluation {
        requested_transition,
        has_ci_evidence,
        has_deploy_evidence,
        target_state,
        block,
    }
}

pub fn build_transition_block_message(
    block_code: &str,
    current_state_name: &str,
    target_state: Option<LinearWorkflowState>,
    ready_for_release_name: &str,
) -> String {
    match block_code {
        "EVIDENCE_REQUIRED" => {
            "status=success requer evidência mínima de CI/checks antes de avançar estado"
                .to_string()
        }
        "INVALID_STATE_TRANSITION" => match target_state {
            Some(target) => format!(
                "transição Linear proibida: '{}' -> '{}'",
                current_state_name,
                workflow_state_label(target, ready_for_release_name)
            ),
            None => "transição Linear proibida".to_string(),
        },
        _ => "transição bloqueada por política".to_string(),
    }
}

pub fn workflow_state_label(state: LinearWorkflowState, ready_for_release_name: &str) -> String {
    match state {
        LinearWorkflowState::Ready => "Ready".to_string(),
        LinearWorkflowState::InProgress => "In Progress".to_string(),
        LinearWorkflowState::ReadyForRelease => ready_for_release_name.to_string(),
        LinearWorkflowState::Done => "Done".to_string(),
        LinearWorkflowState::Unknown => "Unknown".to_string(),
    }
}
