use crate::persistence_rs::JobStatus;

#[derive(Default, Clone)]
pub struct StateMachine;

impl StateMachine {
    pub fn can_transition(&self, from: JobStatus, to: JobStatus) -> bool {
        match from {
            JobStatus::Pending => matches!(to, JobStatus::Planning | JobStatus::Failed),
            JobStatus::Planning => matches!(to, JobStatus::Coding | JobStatus::Failed),
            JobStatus::Coding => matches!(to, JobStatus::Reviewing | JobStatus::Failed),
            JobStatus::Reviewing => matches!(to, JobStatus::Validating | JobStatus::Failed),
            JobStatus::Validating => matches!(to, JobStatus::Committing | JobStatus::Failed),
            JobStatus::Committing => matches!(to, JobStatus::Done | JobStatus::Failed),
            JobStatus::Failed => matches!(to, JobStatus::Pending),
            JobStatus::Done => false,
        }
    }
}
