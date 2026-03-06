use anyhow::Result;

use crate::adapters_rs::GitAdapter;

#[derive(Clone)]
pub struct BranchManager {
    git: GitAdapter,
}

impl BranchManager {
    pub fn new(git: GitAdapter) -> Self {
        Self { git }
    }

    pub async fn create_job_branch(&self, issue_identifier: &str) -> Result<String> {
        let safe = issue_identifier.to_lowercase().replace(' ', "-");
        let branch = format!("code247/{safe}");
        self.git.checkout_new_branch(&branch).await?;
        Ok(branch)
    }

    pub async fn ensure_clean(&self) -> Result<()> {
        self.git.stash_if_needed().await
    }
}
