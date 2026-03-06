# Code247 Tasklist

**Updated:** 2026-03-06

## Status Snapshot (Code247 x Linear)
- [x] Intake endpoint `/intentions` com dedupe + link mapping.
- [x] Readback endpoint `/intentions/{workspace}/{project}`.
- [x] Sync endpoint `/intentions/sync` (comentario + update de status).
- [x] OAuth2 base (`/oauth/start`, `/oauth/callback`, refresh worker).
- [x] Auto-claim Linear -> jobs (polling por estado + idempotencia por issue/job ativo).
- [x] Auto-claim já tenta mover issue para `In Progress` após claim.
- [x] `/intentions/sync` exige evidência mínima (`ci.url` ou `evidence[]`) para mover `Done`.
- [x] `/jobs` protegido por Bearer token (mesma chave operacional de intake).
- [x] Risk scoring determinístico (v1) ligado ao pipeline e persistido em evidência.
- [x] PR risk classificado sem escalada humana obrigatória no merge (no-human middle).
- [x] Auto-merge para PR `light` via GitHub API (aguarda checks verdes + mergeable clean).
- [x] Workflow oficial no Linear aplicado por regra (`Ready`, `In Progress (Code247)`, `Ready for Release`, `Done`).
- [x] Gate de `Done` nivel A/B (deploy evidence) aplicado no runtime.
- [x] Policy light/substantial com risk scoring deterministico no fluxo de PR.
- [x] Politica anti-flaky + red-main automatizada.
- [x] Guardrails de supply chain (workflows/deps/paths sensiveis) ativos.

## H0 Blockers (obrigatorio agora)
- [x] H0-001: Implementar state machine do Linear com transicoes permitidas/proibidas e validação server-side (fecha `G-007`).
- [x] H0-002: Enforçar `Done` somente com evidencia de CI/CD (Nivel A) e usar `Ready for Release` para Nivel B.
- [ ] H0-003: Implementar merge policy `light=auto` e `substantial=auto-gates` com override auditável (auto-merge light já ativo).
- [x] H0-004: Implementar `risk_score` deterministico (pesos do v1.1) e classificar substantial com `score >= 3`.
- [x] H0-005: Enforçar paths sensiveis (`auth/`, `billing/`, `migrations/`, `infra/`, `permissions/`, `.github/workflows/**`) como substantial obrigatorio.
- [x] H0-006: Politica operacional de CI: re-run 1x para flaky; reincidencia => `blocked: ci` + ticket `CI Flaky`.
- [x] H0-007: Politica `red-main` (P0): pausar novas features e priorizar restauração do `main` verde.
- [x] H0-008: Fechar authn/authz das APIs (`/intentions*`, `/jobs`) com JWT + escopo por projeto.
- [x] H0-009: Runner controlado com allowlist de comandos por repo e bloqueio de comandos fora da policy.
- [x] H0-010: Evidencia obrigatoria em PR/issue (`plan`, `AC`, `risk`, `backout`, links de checks/deploy).

### H0-001 decomposicao operacional (state machine + done gate)
- [x] H0-001.1: Definir matriz oficial de transição: `Ready -> In Progress`, `In Progress -> Ready for Release`, `Ready for Release -> Done`.
- [x] H0-001.2: Definir transições proibidas explícitas para `Done` sem prova (`Ready -> Done`, `In Progress -> Done`, `Done -> *`), com código de erro estável.
- [ ] H0-001.3: Consolidar um validador único de transição/evidência e reutilizar em `/intentions/sync`, auto-claim e workers/webhooks.
- [x] H0-001.4: Regra de evidência por estado: `Ready for Release` requer CI (`ci.url` ou equivalente validado); `Done` requer deploy (`deployment_id`/`deploy.url`/equivalente validado).
- [x] H0-001.5: Emitir evento auditável de bloqueio quando transição for negada (`reason_code`, `from_state`, `to_state`, `issue_id`, `request_id`).
- [ ] H0-001.6: Cobrir com testes unitários de matriz + testes de integração de rotas + 1 smoke E2E com caso negativo e positivo (unitários + smoke negativo/positivo prontos; execução integrada com Linear pendente).

## H1 Hardening (proximo passo)
- [ ] H1-001: Evoluir claim para webhook-driven (`issue.ready`/`issue.updated`) mantendo polling como fallback.
- [x] H1-002: Hardening de webhook Linear (HMAC `Linear-Signature`, janela anti-replay, idempotencia por `Linear-Delivery`, DLQ/replay).
- [ ] H1-003: Merge queue + checks em evento `merge_group` para evitar green-on-stale.
- [ ] H1-004: Security scan obrigatorio em repos criticos antes de merge.
- [ ] H1-005: Persistir tracing fim-a-fim (`request_id`, `intention_id`, `issue_id`, `job_id`, `pr_id`, `deploy_id`).
- [ ] H1-006: Retries com backoff + circuit breaker para Linear/GitHub/CI.
- [ ] H1-007: Watchdog de stuck jobs e timeout por etapa (SLA operacional).
- [ ] H1-008: Telemetria de latencia/custo por etapa (`plan`, `code`, `review`, `ci`, `merge`, `deploy`).

## H2 Horizon (governanca + melhoria continua)
- [x] H2-001: Persistir jobs/events/checkpoints no Supabase (`code247_jobs`, `code247_events`, `code247_checkpoints`).
- [x] H2-002: Broadcast de status via Supabase Realtime para `obs-api` e consumidores internos.
- [ ] H2-003: API de status por intenção (`intention_id -> linear/pr/ci/merge/deploy`).
- [ ] H2-004: Policy-as-prompt/code auditavel definindo o que o agente pode tocar por repo/area.
- [ ] H2-005: Feedback loop de rejeicoes (`request changes`) com metricas semanais e ajuste de heuristicas.
- [ ] H2-006: Contrato unico de output para todo ecossistema (`linear`, `ci`, `pr`, `merge`, `deploy`).

## Gaps Tecnicos Imediatos (mapa para H0/H1)
- [ ] GAP-001: Criacao de PR e automacao de review usando `github.rs` com resumo de job/issue/evidencia.
- [ ] GAP-002: Expandir ingestao de contexto do Linear (description, labels, priority, links) antes da execução.
- [ ] GAP-003: Executar operacoes git via `tokio::process::Command` para nao bloquear loop async.

## Done Recently
- [x] Gate fail-closed pré-merge e pré-transição de estado no pipeline: bloqueia avanço quando faltam artefatos mínimos de evidência.
- [x] `/intentions/sync` agora valida transições permitidas/proibidas do workflow Linear (server-side, fail-closed em transição inválida).
- [x] Matriz canônica atualizada para exigir `Ready -> In Progress -> Ready for Release -> Done` (bloqueando `In Progress -> Done`).
- [x] Script operacional `scripts/smoke-p1-state-governance.sh` adicionado para prova de bloqueio de `Done` inválido no fluxo `/intentions -> /intentions/sync`.
- [x] Auth JWT HS256 adicionado para `/jobs` e `/intentions*` com scopes por rota + grants por projeto (`code247_projects` e `code247:project:*`), mantendo fallback legado opcional (`CODE247_AUTH_ALLOW_LEGACY_TOKEN`).
- [x] `/intentions/sync` passou a usar avaliação centralizada de transição/evidência e emitir `code247.intentions.sync.transition_blocked` em bloqueios auditáveis.
- [x] Testes unitários adicionados para grants de projeto, parsing de scopes JWT e cenários soft/hard de bloqueio de transição.
- [x] `TestRunner` com anti-flaky (re-run automático 1x) e bloqueio `red-main` via flag operacional.
- [x] Workflow `.github/workflows/supply-chain-guardrails.yml` ativo para deps/workflows/paths sensíveis com label de revisão obrigatória.
- [x] Pipeline deixou de marcar `Done` no Linear automaticamente (evita `Done` prematuro sem gate externo).
- [x] `.code247/linear-meta.json` para mapear `intention_id -> linear issue`.
- [x] `scripts/publish-intentions.sh` para publicar manifests no intake oficial.
- [x] `scripts/sync-intentions.sh` + `/intentions/sync` para round-trip de execução.
- [x] Auto-claim inicial por polling integrado ao runtime.
- [x] Mirror de eventos de intake/sync para `obs-api` (`OBS_API_BASE_URL` + `OBS_API_TOKEN`) com `trace_id=request_id`.
- [x] Worker de webhook Linear com fila persistente + retry/backoff + DLQ + dedupe por `Linear-Delivery`.
- [x] Pipeline atualiza Linear para `In Progress` no start e `Ready for Release` após merge validado.
- [x] `/intentions/sync` diferencia evidência de CI vs deploy e move `Ready for Release` ou `Done` conforme prova.
- [x] Espelhamento assíncrono SQLite -> Supabase adicionado no runtime (`jobs`, `checkpoints`, `execution events`) com fallback fail-open.
- [x] Broadcast HTTP Realtime por status de job adicionado (`CODE247_SUPABASE_REALTIME_CHANNEL`, padrão `code247:jobs:{tenant_id}`).
- [x] Runner allowlist fail-closed via manifesto (`gates.gate0.commands`) para `cargo check`/`npm run typecheck`.
- [x] Contrato de plano/evidência aplicado em pipeline (`plan_contract`, `acceptance`, `how_to_test`, `backout`, `checks_link`) antes de auto-merge e transição de estado.
