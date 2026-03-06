use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::adapters_rs::{LinearAdapter, LinearIssue};

#[derive(Clone)]
pub struct ContextBuilder {
    spec_path: PathBuf,
    linear: LinearAdapter,
}

impl ContextBuilder {
    pub fn new(spec_path: impl Into<PathBuf>, linear: LinearAdapter) -> Self {
        Self {
            spec_path: spec_path.into(),
            linear,
        }
    }

    pub async fn build_planning_prompt(
        &self,
        issue_id: &str,
        fallback_payload: &str,
    ) -> Result<String> {
        let issue = self.linear.get_issue(issue_id).await?;
        Ok(self.compose(&issue, fallback_payload))
    }

    fn compose(&self, issue: &LinearIssue, fallback_payload: &str) -> String {
        let spec = fs::read_to_string(&self.spec_path).unwrap_or_else(|_| canonical_spec_stub());
        let description = issue
            .description
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("(sem description na issue)");
        format!(
            "Issue: {} - {}\nEstado: {}\nDescription: {}\nPayload original: {}\n\n{}\n\nInstrução: gere um plano técnico executável e objetivo.",
            issue.identifier,
            issue.title,
            issue.state.name,
            description,
            fallback_payload,
            spec,
        )
    }
}

fn canonical_spec_stub() -> String {
    "=== PLATAFORMA VOULEZVOUS — CONTEXTO OBRIGATÓRIO ===\nSTACK TÉCNICA: Rust + TypeScript/React + Supabase JWT + Postgres + Realtime\n10 INVARIANTES GLOBAIS: transição=sessão nova, consentimento obrigatório, thumbs sem áudio, viewport gating, config source of truth, schema first, login->party, ownercard auditável, sem vazamento entre sessões, privacidade por padrão.\n=================================================".to_string()
}
