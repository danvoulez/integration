# Dual-Agents v1.2 (Rust-only)

Sistema fortalecido para produção e **100% em Rust**.

## O que está em produção

- Control plane com scheduler e máquina de estados
- Pipeline completo e idempotente: plan → code → review → validate → commit
- API operacional com endpoints de health e listagem de jobs
- Persistência em SQLite (jobs, checkpoints e execution log)
- Espelhamento opcional para Supabase (`code247_jobs`, `code247_checkpoints`, `code247_events`) + broadcast Realtime
- Evidências gravadas em filesystem
- Adapters reais para Anthropic, Ollama, Git e Linear (GraphQL)
- Commit + push automático para o remote/branch configurado no ambiente

## Executar localmente

```bash
cargo run
```

## Validação de manifesto de projeto

Code247 valida o manifesto `.code247/workspace.manifest.json` no startup (runtime), quando habilitado.

- `CODE247_MANIFEST_PATH` (default: `.code247/workspace.manifest.json`)
- `CODE247_RUNNER_ALLOWLIST_ENABLED` (default: `true`) ativa bloqueio fail-closed de comandos do runner fora da allowlist
- `CODE247_RUNNER_ALLOWLIST_MANIFEST_PATH` (default: mesmo valor de `CODE247_MANIFEST_PATH`; lê `gates.gate0.commands`)
- `CODE247_MANIFEST_SCHEMA_PATH` (default: `schemas/workspace.manifest.schema.json`)
- `CODE247_MANIFEST_REQUIRED` (default: `false`)

Validação manual (local/CI):

```bash
cargo run --bin validate_manifest
```

Validação do contrato de extensões Linear (`x_*`):

```bash
./scripts/validate-linear-extensions.sh
```

## Endpoints

- `GET /health` → `https://code247.logline.world/health` (prod) / `http://localhost:4001/health` (local)
- `GET /jobs` → `https://code247.logline.world/jobs` (prod) / `http://localhost:4001/jobs` (local) — Bearer obrigatório (`code247:jobs:read`)
- `GET /oauth/start` → inicia OAuth2 no Linear (`actor=app`)
- `GET /oauth/callback` → callback de autorização OAuth2
- `GET /oauth/status` → status de conexão OAuth2 (configurado/conectado)
- `POST /intentions` → intake oficial (`manifest.intentions`) com sync Linear + resposta de linkage (`code247:intentions:write` + escopo de projeto)
- `POST /intentions/sync` → publica estado/evidências de execução no Linear (comentário + Done opcional com evidência obrigatória) (`code247:intentions:sync` + escopo de projeto)
- `GET /intentions/{workspace}/{project}` → snapshot do linkage e último ingest (vai‑e‑volta) (`code247:intentions:read` + escopo de projeto)

## Configuração por ambiente

Todas as configurações foram centralizadas no arquivo `.env`.

Em ambiente real, os secrets devem ser injetados via `Doppler`:

```bash
cd code247.logline.world
doppler run --project logline-ecosystem --config dev -- cargo run
```

`.env` local continua aceitável apenas para dev/teste isolado.

- `DB_PATH` (default: `dual_agents.db`)
- `EVIDENCE_PATH` (default: `evidence`)
- `REPO_ROOT` (default: `.`)
- `GIT_BRANCH` (default: `main`)
- `GIT_REMOTE` (default: `origin`)
- `ANTHROPIC_MODEL` (default: `claude-3-5-sonnet-20241022`)
- `ANTHROPIC_API_KEY` (opcional; sem chave usa fallback local no planning/review)
- `OLLAMA_MODEL` (default: `codellama`)
- `OLLAMA_BASE_URL` (default: `http://localhost:11434`)
- `LINEAR_API_KEY` (opcional; necessário para fluxo legacy GraphQL sem OAuth)
- `LINEAR_API_BASE_URL` (opcional; default `https://api.linear.app`, útil para smoke/local mock)
- `LINEAR_CLIENT_ID` (OAuth app)
- `LINEAR_CLIENT_SECRET` (OAuth app)
- `LINEAR_OAUTH_BASE_URL` (opcional; default `https://linear.app`, útil para smoke/local mock)
- `LINEAR_OAUTH_REDIRECT_URI` (callback OAuth)
- `LINEAR_OAUTH_SCOPES` (default: `read write comments:create issues:create`)
- `LINEAR_OAUTH_ACTOR` (default: `app`)
- `LINEAR_OAUTH_STATE_TTL_SECONDS` (default: `600`)
- `LINEAR_OAUTH_REFRESH_LEAD_SECONDS` (default: `300`)
- `LINEAR_OAUTH_REFRESH_INTERVAL_SECONDS` (default: `60`)
- `LINEAR_CLAIM_ENABLED` (default: `true`) ativa auto-claim de issues do Linear para jobs locais
- `LINEAR_CLAIM_STATE_NAME` (default: `Ready`) estado Linear monitorado para claim
- `LINEAR_CLAIM_IN_PROGRESS_STATE_NAME` (default: `In Progress`) estado aplicado após claim bem sucedido
- `LINEAR_CLAIM_INTERVAL_SECONDS` (default: `20`) intervalo do claimer
- `LINEAR_CLAIM_MAX_PER_CYCLE` (default: `25`) limite de claims por ciclo
- `LINEAR_WEBHOOK_SIGNING_SECRET` (assinatura webhook)
- `LINEAR_TEAM_ID` (**obrigatório**)
- `LINEAR_DONE_STATE_TYPE` (default: `completed`)
- `CODE247_PUBLIC_URL` (default: `https://code247.logline.world`)
- `CODE247_INTENTIONS_TOKEN` (fallback legado; opcional quando JWT está ativo)
- `CODE247_AUTH_ALLOW_LEGACY_TOKEN` (default: `false`; manter `true` apenas como escape hatch temporário de migração)
- `SUPABASE_JWT_SECRET` (validação principal de Bearer JWT HS256)
- `SUPABASE_JWT_SECRET_LEGACY` (fallback para rotação de segredo JWT)
- `SUPABASE_JWT_AUDIENCE` (opcional; valida `aud`)
- `CODE247_SCOPE_JOBS_READ` (default: `code247:jobs:read`)
- `CODE247_SCOPE_JOBS_WRITE` (default: `code247:jobs:write`)
- `CODE247_SCOPE_INTENTIONS_WRITE` (default: `code247:intentions:write`)
- `CODE247_SCOPE_INTENTIONS_SYNC` (default: `code247:intentions:sync`)
- `CODE247_SCOPE_INTENTIONS_READ` (default: `code247:intentions:read`)
- `CODE247_SCOPE_ADMIN` (default: `code247:admin`; bypass controlado de escopo/projeto)
- `CODE247_LINEAR_META_PATH` (default: `.code247/linear-meta.json`)
- `CODE247_SUPABASE_SYNC_ENABLED` (default: `true`) liga espelhamento SQLite -> Supabase
- `SUPABASE_URL` (URL do projeto Supabase para PostgREST/Realtime)
- `SUPABASE_SERVICE_ROLE_KEY` (chave service role usada pelo worker de sync)
- `CODE247_SUPABASE_TENANT_ID` (tenant_id fixo para registros `code247_*`)
- `CODE247_SUPABASE_APP_ID` (app_id fixo para registros `code247_*`)
- `CODE247_SUPABASE_USER_ID` (opcional; user_id service account para trilha de auditoria)
- `CODE247_SUPABASE_REALTIME_ENABLED` (default: `true`) envia broadcast HTTP de status de job
- `CODE247_SUPABASE_REALTIME_CHANNEL` (default: `code247:jobs:{tenant_id}`)
- `OBS_API_BASE_URL` (opcional; quando definido, espelha eventos de intake/sync para `/api/v1/events/ingest`)
- `OBS_API_TOKEN` (opcional; Bearer para ingest no obs-api)
- `HEALTH_PORT` (default: `4001`)
- `POLL_INTERVAL_MS` (default: `1000`)
- `MAX_REVIEW_ITERATIONS` (default: `2`)
- `CODE247_STAGE_LEASE_OWNER` (default: UUID por processo) identificador do worker que segura lease de etapa
- `CODE247_STAGE_LEASE_SWEEP_INTERVAL_SECONDS` (default: `30`) intervalo do sweeper lateral que aplica expiração de lease
- `CODE247_STAGE_TIMEOUT_PLANNING_SECONDS` (default: `900`)
- `CODE247_STAGE_TIMEOUT_CODING_SECONDS` (default: `1800`)
- `CODE247_STAGE_TIMEOUT_REVIEWING_SECONDS` (default: `900`)
- `CODE247_STAGE_TIMEOUT_VALIDATING_SECONDS` (default: `1200`)
- `CODE247_STAGE_TIMEOUT_COMMITTING_SECONDS` (default: `2100`)
- `CODE247_MANIFEST_PATH` (default: `.code247/workspace.manifest.json`)
- `CODE247_MANIFEST_SCHEMA_PATH` (default: `schemas/workspace.manifest.schema.json`)
- `CODE247_MANIFEST_REQUIRED` (default: `false`)
- `GITHUB_AUTO_MERGE_ENABLED` (default: `true`) habilita merge automático em PRs `light`
- `GITHUB_AUTO_MERGE_TIMEOUT_SECONDS` (default: `1800`) janela máxima de espera por checks verdes
- `GITHUB_AUTO_MERGE_POLL_SECONDS` (default: `20`) intervalo de polling de status/mergeabilidade

Regras operacionais atuais:
- Auth JWT aceita `scope`/`scopes` (string/array) e grants de projeto em `code247_projects` ou scopes `code247:project:<workspace>/<project>` (`workspace/*` e `*` suportados).
- O pipeline de código **não move mais a issue para Done** automaticamente ao abrir/mergear PR.
- `POST /intentions/sync` só move para Done quando `status=success` **e** existe evidência (`ci.url` ou `evidence[]`).
- Verificação oficial de governança de estado: `../scripts/verify-code247-state-governance.sh` roda os testes HTTP integrados de `/intentions` + `/intentions/sync` com mock local do Linear, cobrindo:
  - `In Progress -> Ready for Release` com evidência de CI
  - bloqueio de `In Progress -> Done` mesmo com deploy
  - `Ready for Release -> Done` com evidência de deploy
- Pipeline calcula `risk_score` determinístico por PR e grava evidência `risk`.
- PR classificado como `substantial` aplica gates automatizados mais rígidos (cloud/no-merge), sem exigir human merge por padrão.
- PR classificado como `light` tenta auto-merge no GitHub assim que checks estiverem verdes e `mergeable_state=clean`.
- Claim de `PENDING` agora é atômico no SQLite e já promove o job para `PLANNING` com lease; isso evita double execution entre instâncias antes do pipeline começar.
- Etapas `PLANNING/CODING/REVIEWING/VALIDATING/COMMITTING` agora carregam `lease_expires_at` + `heartbeat_at`; expiração de lease vira transição fail-closed normal para `FAILED`, com log/evidência/comentário assíncrono.
- Pipeline fail-closed exige contrato de plano e evidências antes de merge/transição de estado: `plan_contract`, `acceptance`, `how_to_test`, `backout`, `validation`, `risk`, `pr`, `checks_link`.

Smoke operacional recomendado para a camada de lease:

```bash
DB_PATH=/abs/path/to/dual_agents.db ./scripts/smoke-code247-stage-lease.sh
```

O smoke injeta um job sintético em `CODING` com `lease_expires_at` já vencida e valida três invariantes:
- transição deterministicamente fail-closed para `FAILED`
- emissão de exatamente um evento `lease_expired` no `execution_log`
- ausência de comentário Linear para issues `smoke:*`

## Documentação Linear

- `docs/Code247-Linear-Integration.md`
- `docs/Linear-Setup-Runbook.md`
- `docs/Linear-Implementation-Checklist.md`
- `docs/linear-tasklist.json`

## Deploy

- Docker multi-stage build em `docker/Dockerfile`
- Service unit Rust em `systemd/dual-agents.service`
