use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use tokio::process::Command;
use urlencoding::encode;

#[derive(Clone)]
pub struct AnthropicAdapter {
    model: String,
    api_key: Option<String>,
    http: Client,
}

impl AnthropicAdapter {
    pub fn new(model: String, api_key: Option<String>) -> Self {
        Self {
            model,
            api_key,
            http: Client::new(),
        }
    }

    pub async fn plan(&self, prompt: &str) -> Result<String> {
        let Some(api_key) = &self.api_key else {
            return Ok(format!("Plano local (fallback): {}", prompt));
        };

        let req = json!({
            "model": self.model,
            "max_tokens": 1200,
            "messages": [{"role":"user","content": format!("Crie um plano estruturado e objetivo para implementar: {prompt}")}]
        });

        let response: AnthropicMessageResponse = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&req)
            .send()
            .await
            .context("falha ao chamar Anthropic para planejamento")?
            .error_for_status()
            .context("Anthropic retornou erro em planejamento")?
            .json()
            .await
            .context("resposta Anthropic inválida (planning)")?;

        Ok(response.concat_text())
    }

    pub async fn review(&self, code: &str) -> Result<ReviewOutput> {
        let Some(api_key) = &self.api_key else {
            return Ok(ReviewOutput {
                summary: "Review local (fallback) sem issues críticas".to_string(),
                issues: vec![],
                code: code.to_string(),
            });
        };

        let req = json!({
            "model": self.model,
            "max_tokens": 1600,
            "messages": [{"role":"user","content": format!(
                "Revise o código abaixo e retorne JSON com campos: \
                summary (string), \
                issues (array de {{severity: string, message: string}}), \
                code (string com o código corrigido). \
                IMPORTANTE: o campo 'code' DEVE preservar exatamente o formato de blocos \
                <file path=\"caminho/do/arquivo\">...</file> — não remova, não substitua, não resuma esses blocos. \
                Corrija apenas o conteúdo interno de cada bloco se necessário. \
                Código a revisar:\n{code}"
            )}]
        });

        let response: AnthropicMessageResponse = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&req)
            .send()
            .await
            .context("falha ao chamar Anthropic para review")?
            .error_for_status()
            .context("Anthropic retornou erro em review")?
            .json()
            .await
            .context("resposta Anthropic inválida (review)")?;

        let text = response.concat_text();
        serde_json::from_str::<ReviewOutput>(&text).or_else(|_| {
            Ok(ReviewOutput {
                summary: text,
                issues: vec![],
                code: code.to_string(),
            })
        })
    }
}

#[derive(Clone)]
pub struct OllamaAdapter {
    model: String,
    base_url: String,
    http: Client,
}

impl OllamaAdapter {
    pub fn new(model: String, base_url: String) -> Self {
        Self {
            model,
            base_url,
            http: Client::new(),
        }
    }

    pub async fn code(&self, plan: &str) -> Result<String> {
        let req = json!({
            "model": self.model,
            "prompt": format!(
                "Implemente o plano abaixo em código real, sem stubs. \
        Retorne SOMENTE blocos de arquivo neste formato exato, sem texto fora dos blocos:\n\
        <file path=\"src/components/UserCard.tsx\">\n... código ...\n</file>\n\nPlano:\n{plan}"
            ),
            "stream": false
        });

        let response: OllamaGenerateResponse = self
            .http
            .post(format!(
                "{}/api/generate",
                self.base_url.trim_end_matches('/')
            ))
            .json(&req)
            .send()
            .await
            .context("falha ao chamar Ollama")?
            .error_for_status()
            .context("Ollama retornou erro")?
            .json()
            .await
            .context("resposta Ollama inválida")?;

        Ok(response.response)
    }
}

// ============================================================================
// LlmGatewayAdapter - Unified adapter for all LLM calls via llm-gateway
// ============================================================================

/// OpenAI-compatible chat response from llm-gateway
#[derive(Debug, Deserialize)]
struct GatewayResponse {
    choices: Vec<GatewayChoice>,
    usage: Option<GatewayUsage>,
}

#[derive(Debug, Deserialize)]
struct GatewayChoice {
    message: GatewayMessage,
}

#[derive(Debug, Deserialize)]
struct GatewayMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct GatewayUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

/// Unified LLM adapter that routes all calls through llm-gateway.
/// This ensures proper fuel tracking, cost optimization, and observability.
#[derive(Clone)]
pub struct LlmGatewayAdapter {
    gateway_url: String,
    api_key: String,
    http: Client,
}

impl LlmGatewayAdapter {
    pub fn new(gateway_url: String, api_key: String) -> Self {
        Self {
            gateway_url,
            api_key,
            http: Client::new(),
        }
    }

    /// Plan stage - uses "genius" mode for best reasoning (Claude/GPT-4 class)
    pub async fn plan(&self, prompt: &str) -> Result<String> {
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "Crie um plano estruturado e objetivo para implementar: {prompt}"
            )
        })];

        self.call("genius", messages, 1200).await
    }

    /// Code stage - uses "code" mode (local Ollama first, premium fallback)
    pub async fn code(&self, plan: &str) -> Result<String> {
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "Implemente o plano abaixo em código real, sem stubs. \
        Retorne SOMENTE blocos de arquivo neste formato exato, sem texto fora dos blocos:\n\
        <file path=\"src/components/UserCard.tsx\">\n... código ...\n</file>\n\nPlano:\n{plan}"
            )
        })];

        self.call("code", messages, 4096).await
    }

    /// Review stage - uses "genius" mode for thorough code review
    pub async fn review(&self, code: &str) -> Result<ReviewOutput> {
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "Revise o código abaixo e retorne JSON com campos: \
        summary (string), \
        issues (array de {{severity: string, message: string}}), \
        code (string com o código corrigido). \
        IMPORTANTE: o campo 'code' DEVE preservar exatamente o formato de blocos \
        <file path=\"caminho/do/arquivo\">...</file> — não remova, não substitua, não resuma esses blocos. \
        Corrija apenas o conteúdo interno de cada bloco se necessário. \
        Código a revisar:\n{code}"
            )
        })];

        let text = self.call("genius", messages, 4096).await?;

        serde_json::from_str::<ReviewOutput>(&text).or_else(|_| {
            Ok(ReviewOutput {
                summary: text,
                issues: vec![],
                code: code.to_string(),
            })
        })
    }

    /// Cloud re-evaluation gate for substantial PRs.
    /// Fail-closed: invalid/malformed output is treated as NO.
    pub async fn cloud_pr_risk_decision(
        &self,
        context: serde_json::Value,
    ) -> Result<CloudGateDecision> {
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "Avalie o risco de merge deste PR substantial e responda SOMENTE JSON válido com este schema:\n\
        {{\"decision\":\"YES|NO|CLOUD\",\"confidence\":0.0,\"reason_codes\":[\"...\"],\"rationale\":\"...\"}}\n\
        Regras:\n\
        - YES: pode auto-mergear após checks verdes.\n\
        - NO: não pode auto-mergear.\n\
        - CLOUD: precisa de reavaliação cloud mais forte/gates adicionais.\n\
        Contexto:\n{}",
                serde_json::to_string_pretty(&context)?
            )
        })];

        let text = match self.call("genius", messages, 800).await {
            Ok(raw) => raw,
            Err(err) => {
                return Ok(CloudGateDecision::deny(
                    "cloud_gate_call_failed",
                    format!("falha ao chamar cloud gate: {err}"),
                ));
            }
        };

        Ok(parse_cloud_gate_decision(&text).unwrap_or_else(|| {
            CloudGateDecision::deny(
                "cloud_gate_invalid_payload",
                "cloud gate retornou payload inválido".to_string(),
            )
        }))
    }

    /// Internal call to llm-gateway
    async fn call(
        &self,
        mode: &str,
        messages: Vec<serde_json::Value>,
        max_tokens: u32,
    ) -> Result<String> {
        let url = format!(
            "{}/v1/chat/completions",
            self.gateway_url.trim_end_matches('/')
        );

        let req = json!({
            "messages": messages,
            "max_tokens": max_tokens,
            "mode": mode,
            "stream": false,
            "task_hint": match mode {
                "genius" => "planning",
                "code" => "coding",
                _ => "general"
            }
        });

        let response: GatewayResponse = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .context("falha ao chamar llm-gateway")?
            .error_for_status()
            .context("llm-gateway retornou erro")?
            .json()
            .await
            .context("resposta llm-gateway inválida")?;

        response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("llm-gateway retornou choices vazio"))
    }
}

#[derive(Clone)]
pub struct GitAdapter {
    repo_root: String,
    branch: String,
    remote: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommitOutput {
    pub sha: String,
    pub branch: String,
}

impl GitAdapter {
    pub fn new(repo_root: String, branch: String, remote: String) -> Self {
        Self {
            repo_root,
            branch,
            remote,
        }
    }

    pub async fn changed_files(&self) -> Result<Vec<String>> {
        let output = self.git_async(["status", "--porcelain"]).await?;
        Ok(output
            .lines()
            .filter_map(|line| line.get(3..).map(ToString::to_string))
            .collect())
    }

    pub async fn commit(
        &self,
        job_id: &str,
        title: &str,
        files: &[String],
        summary: &str,
    ) -> Result<CommitOutput> {
        if files.is_empty() {
            bail!("nenhum arquivo alterado para commit");
        }

        for file in files {
            self.git_async(["add", "--", file]).await?;
        }

        let message = format!("{title}\n\njob: {job_id}\n\n{summary}");
        self.git_async(["commit", "-m", &message]).await?;

        let sha = self.git_async(["rev-parse", "HEAD"]).await?;
        let branch = self
            .git_async(["rev-parse", "--abbrev-ref", "HEAD"])
            .await?;
        Ok(CommitOutput {
            sha: sha.trim().to_string(),
            branch: branch.trim().to_string(),
        })
    }

    pub async fn checkout_new_branch(&self, branch: &str) -> Result<()> {
        self.git_async(["checkout", &self.branch]).await?;
        self.git_async(["checkout", "-B", branch]).await?;
        Ok(())
    }

    pub async fn stash_if_needed(&self) -> Result<()> {
        let status = self.git_async(["status", "--porcelain"]).await?;
        if !status.trim().is_empty() {
            let _ = self
                .git_async(["stash", "push", "-u", "-m", "code247-auto-stash"])
                .await?;
        }
        Ok(())
    }

    pub async fn push_branch(&self, branch: &str) -> Result<()> {
        self.git_async(["push", &self.remote, branch]).await?;
        Ok(())
    }

    pub async fn diff_lines_for_commit(&self, sha: &str) -> Result<usize> {
        let output = self
            .git_async(["show", "--numstat", "--format=", sha])
            .await?;
        let mut total: usize = 0;
        for line in output.lines() {
            let mut parts = line.split_whitespace();
            let added = parts.next();
            let deleted = parts.next();
            if let (Some(a), Some(d)) = (added, deleted) {
                // Binary entries can show "-" in numstat; skip those.
                if let (Ok(a_num), Ok(d_num)) = (a.parse::<usize>(), d.parse::<usize>()) {
                    total = total.saturating_add(a_num.saturating_add(d_num));
                }
            }
        }
        Ok(total)
    }

    pub async fn git_async<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        let out = Command::new("git")
            .current_dir(&self.repo_root)
            .args(args)
            .output()
            .await
            .with_context(|| format!("falha executando git {:?}", args))?;

        if !out.status.success() {
            return Err(anyhow!(
                "git {:?} falhou: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

#[derive(Clone)]
pub struct LinearOAuthClient {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    scopes: String,
    actor: String,
    http: Client,
}

impl LinearOAuthClient {
    pub fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        scopes: String,
        actor: String,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
            scopes,
            actor,
            http: Client::new(),
        }
    }

    pub fn authorize_url(&self, state: &str) -> String {
        format!(
            "https://linear.app/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&actor={}",
            encode(&self.client_id),
            encode(&self.redirect_uri),
            encode(&self.scopes),
            encode(state),
            encode(&self.actor),
        )
    }

    pub async fn exchange_code(&self, code: &str) -> Result<LinearOAuthTokenResponse> {
        let form = [
            ("grant_type", "authorization_code"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("redirect_uri", &self.redirect_uri),
            ("code", code),
        ];
        self.token_request(&form).await
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<LinearOAuthTokenResponse> {
        let form = [
            ("grant_type", "refresh_token"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("refresh_token", refresh_token),
        ];
        self.token_request(&form).await
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<LinearOAuthTokenResponse> {
        let response = self
            .http
            .post("https://api.linear.app/oauth/token")
            .form(form)
            .send()
            .await
            .context("falha ao chamar endpoint OAuth token do Linear")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("falha ao ler resposta OAuth do Linear")?;
        if !status.is_success() {
            bail!("Linear OAuth token retornou {status}: {body}");
        }

        serde_json::from_str::<LinearOAuthTokenResponse>(&body)
            .context("payload OAuth token do Linear inválido")
    }
}

#[derive(Clone)]
pub struct LinearAdapter {
    api_key: String,
    team_id: String,
    http: Client,
}

impl LinearAdapter {
    pub fn new(api_key: String, team_id: String) -> Self {
        Self {
            api_key,
            team_id,
            http: Client::new(),
        }
    }

    pub async fn get_issue(&self, issue_id: &str) -> Result<LinearIssue> {
        self.graphql(
            r#"query($id:String!){issue(id:$id){id identifier title description state{id name type}}}"#,
            json!({"id": issue_id}),
        )
        .await
    }

    pub async fn list_team_issues(&self, state_name: Option<&str>) -> Result<Vec<LinearIssue>> {
        let result: LinearIssuesResult = if let Some(state_name) = state_name {
            self.graphql(
                r#"
                    query($teamId:String!, $stateName:String!){
                      issues(filter:{team:{id:{eq:$teamId}}, state:{name:{eq:$stateName}}}){
                        nodes{ id identifier title description state{id name type} }
                      }
                    }
                "#,
                json!({"teamId": self.team_id, "stateName": state_name}),
            )
            .await?
        } else {
            self.graphql(
                r#"
                    query($teamId:String!){
                      issues(filter:{team:{id:{eq:$teamId}}}){
                        nodes{ id identifier title description state{id name type} }
                      }
                    }
                "#,
                json!({"teamId": self.team_id}),
            )
            .await?
        };
        Ok(result.issues.nodes)
    }

    pub async fn update_issue_state(&self, issue_id: &str, state_id: &str) -> Result<()> {
        let result: MutationOk = self
            .graphql(
                r#"mutation($id:String!, $stateId:String!){issueUpdate(id:$id,input:{stateId:$stateId}){success}}"#,
                json!({"id": issue_id, "stateId": state_id}),
            )
            .await?;
        if !result.issue_update.success {
            bail!("Linear não confirmou sucesso ao atualizar issue {issue_id}");
        }
        Ok(())
    }

    pub async fn bulk_update_issue_state(
        &self,
        issue_ids: &[String],
        state_id: &str,
    ) -> Result<()> {
        for issue_id in issue_ids {
            self.update_issue_state(issue_id, state_id).await?;
        }
        Ok(())
    }

    pub async fn find_state_id_by_type(&self, state_type: &str) -> Result<String> {
        let result: WorkflowStatesResult = self
            .graphql(
                r#"query($teamId:String!){team(id:$teamId){states{id name type}}}"#,
                json!({"teamId": self.team_id}),
            )
            .await?;
        result
            .team
            .states
            .into_iter()
            .find(|s| s.r#type.eq_ignore_ascii_case(state_type))
            .map(|s| s.id)
            .ok_or_else(|| anyhow!("estado Linear do tipo {state_type} não encontrado"))
    }

    pub async fn find_state_id_by_name(&self, state_name: &str) -> Result<String> {
        let result: WorkflowStatesResult = self
            .graphql(
                r#"query($teamId:String!){team(id:$teamId){states{id name type}}}"#,
                json!({"teamId": self.team_id}),
            )
            .await?;
        result
            .team
            .states
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(state_name))
            .map(|s| s.id)
            .ok_or_else(|| anyhow!("estado Linear com nome {state_name} não encontrado"))
    }

    pub async fn create_issue(
        &self,
        title: &str,
        description: &str,
        priority: i32,
    ) -> Result<LinearIssueRef> {
        let result: IssueMutationResult = self
            .graphql(
                r#"mutation($teamId:String!, $title:String!, $description:String!, $priority:Float!){
                    issueCreate(input:{teamId:$teamId, title:$title, description:$description, priority:$priority}){
                      success
                      issue { id identifier }
                    }
                }"#,
                json!({
                    "teamId": self.team_id,
                    "title": title,
                    "description": description,
                    "priority": priority,
                }),
            )
            .await?;
        if !result.issue_create.success {
            bail!("Linear não confirmou sucesso ao criar issue");
        }
        result
            .issue_create
            .issue
            .ok_or_else(|| anyhow!("Linear não retornou issue criada"))
    }

    pub async fn update_issue(
        &self,
        issue_id: &str,
        title: &str,
        description: &str,
        priority: i32,
    ) -> Result<LinearIssueRef> {
        let result: IssueUpdateResult = self
            .graphql(
                r#"mutation($id:String!, $title:String!, $description:String!, $priority:Float!){
                    issueUpdate(id:$id, input:{title:$title, description:$description, priority:$priority}){
                      success
                      issue { id identifier }
                    }
                }"#,
                json!({
                    "id": issue_id,
                    "title": title,
                    "description": description,
                    "priority": priority,
                }),
            )
            .await?;
        if !result.issue_update.success {
            bail!("Linear não confirmou sucesso ao atualizar issue {issue_id}");
        }
        result
            .issue_update
            .issue
            .ok_or_else(|| anyhow!("Linear não retornou issue atualizada"))
    }

    pub async fn create_comment(&self, issue_id: &str, body: &str) -> Result<()> {
        let result: CommentMutationResult = self
            .graphql(
                r#"mutation($input: CommentCreateInput!){
                    commentCreate(input:$input){
                      success
                    }
                }"#,
                json!({
                    "input": {
                        "issueId": issue_id,
                        "body": body,
                    }
                }),
            )
            .await?;
        if !result.comment_create.success {
            bail!("Linear não confirmou sucesso ao criar comentário na issue {issue_id}");
        }
        Ok(())
    }

    async fn graphql<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let response: GraphqlEnvelope<T> = self
            .http
            .post("https://api.linear.app/graphql")
            .bearer_auth(&self.api_key)
            .json(&json!({"query": query, "variables": variables}))
            .send()
            .await
            .context("falha ao chamar Linear")?
            .error_for_status()
            .context("Linear retornou HTTP error")?
            .json()
            .await
            .context("resposta Linear inválida")?;

        if let Some(errors) = response.errors {
            let joined = errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            bail!("erro GraphQL Linear: {joined}");
        }
        response
            .data
            .ok_or_else(|| anyhow!("Linear retornou data vazia"))
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewIssue {
    pub severity: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ReviewOutput {
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudGateDecision {
    pub decision: String,
    pub confidence: f64,
    pub reason_codes: Vec<String>,
    pub rationale: String,
}

impl CloudGateDecision {
    fn deny(code: &str, rationale: String) -> Self {
        Self {
            decision: "NO".to_string(),
            confidence: 0.0,
            reason_codes: vec![code.to_string()],
            rationale,
        }
    }

    pub fn is_yes(&self) -> bool {
        self.decision.eq_ignore_ascii_case("YES")
    }
}

fn parse_cloud_gate_decision(raw: &str) -> Option<CloudGateDecision> {
    let candidates = [
        raw.trim().to_string(),
        raw.trim()
            .strip_prefix("```json")
            .and_then(|s| s.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        raw.trim()
            .strip_prefix("```")
            .and_then(|s| s.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        extract_json_object(raw).unwrap_or_default(),
    ];

    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        if let Ok(mut parsed) = serde_json::from_str::<CloudGateDecision>(&candidate) {
            parsed.decision = parsed.decision.to_ascii_uppercase();
            if parsed.decision != "YES" && parsed.decision != "NO" && parsed.decision != "CLOUD" {
                parsed.decision = "NO".to_string();
            }
            if !parsed.confidence.is_finite() {
                parsed.confidence = 0.0;
            }
            parsed.confidence = parsed.confidence.clamp(0.0, 1.0);
            if parsed.reason_codes.is_empty() {
                parsed.reason_codes = vec!["cloud_gate_no_reason".to_string()];
            }
            return Some(parsed);
        }
    }
    None
}

fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end < start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

#[cfg(test)]
mod cloud_gate_tests {
    use super::parse_cloud_gate_decision;

    #[test]
    fn parses_plain_json_payload() {
        let raw = r#"{"decision":"YES","confidence":0.91,"reason_codes":["blast_radius_low"],"rationale":"safe"}"#;
        let decision = parse_cloud_gate_decision(raw).expect("must parse");
        assert_eq!(decision.decision, "YES");
        assert!(decision.confidence > 0.9);
    }

    #[test]
    fn parses_fenced_json_payload() {
        let raw = "```json\n{\"decision\":\"no\",\"confidence\":0.2,\"reason_codes\":[\"tests_missing\"],\"rationale\":\"insufficient\"}\n```";
        let decision = parse_cloud_gate_decision(raw).expect("must parse");
        assert_eq!(decision.decision, "NO");
        assert_eq!(decision.reason_codes, vec!["tests_missing".to_string()]);
    }

    #[test]
    fn returns_none_for_invalid_payload() {
        assert!(parse_cloud_gate_decision("not-json").is_none());
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicContent>,
}

impl AnthropicMessageResponse {
    fn concat_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct GraphqlEnvelope<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub state: LinearState,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinearState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(Debug, Deserialize)]
struct LinearIssuesResult {
    issues: LinearIssueNodes,
}

#[derive(Debug, Deserialize)]
struct LinearIssueNodes {
    nodes: Vec<LinearIssue>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStatesResult {
    team: TeamStates,
}

#[derive(Debug, Deserialize)]
struct TeamStates {
    states: Vec<LinearState>,
}

#[derive(Debug, Deserialize)]
struct MutationOk {
    #[serde(rename = "issueUpdate")]
    issue_update: MutationSuccess,
}

#[derive(Debug, Deserialize)]
struct MutationSuccess {
    success: bool,
}

#[derive(Debug, Deserialize)]
struct IssueMutationResult {
    #[serde(rename = "issueCreate")]
    issue_create: IssueMutationSuccess,
}

#[derive(Debug, Deserialize)]
struct IssueUpdateResult {
    #[serde(rename = "issueUpdate")]
    issue_update: IssueMutationSuccess,
}

#[derive(Debug, Deserialize)]
struct IssueMutationSuccess {
    success: bool,
    issue: Option<LinearIssueRef>,
}

#[derive(Debug, Deserialize)]
struct CommentMutationResult {
    #[serde(rename = "commentCreate")]
    comment_create: MutationSuccess,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinearIssueRef {
    pub id: String,
    pub identifier: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LinearOAuthTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_in: i64,
}
