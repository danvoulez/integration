# Tasklist Geral do Ecossistema

**Updated:** 2026-03-07  
**Foco do ciclo atual:** hardening backend + fechar lacunas de integração e debug (`code247`, `edge-control`, `llm-gateway`, `fuel/obs-api`, `logic/CLI`)  
**Regra do ciclo:** UI por último, depois de contratos/gates/testes severos estarem verdes.

## 1) Backlog Ativo (somente pendências)

### 1.1 P0 - Bloqueios para Code247 aceitar carga séria
- [x] C247-RDY-001: Fechar authn/authz completo em `/intentions*`, `/jobs`, webhooks e workers com JWT + project scope (substituir token único legado).
- [ ] C247-RDY-007: Ativar merge queue (`merge_group`) em repositórios críticos para evitar green-on-stale.
- [ ] C247-RDY-008: Tornar security scan obrigatório para repos críticos antes de merge substantial.
- [x] C247-RDY-011: Expor telemetria de latência/custo por etapa (`plan/code/review/ci/merge/deploy`) no obs-api.

### 1.2 P0 - Hardening central (cross-app)
- [ ] G-001: Fechar round-trip único `manifest.intentions -> Code247 -> Linear -> pipeline -> PR/merge -> deploy` com rastreabilidade fim-a-fim.
- [ ] G-004: Padronizar auth service-to-service única (JWT + escopo de projeto) em todos os serviços centrais.
- [ ] G-007: Fechar enforcement completo de `Done` sem bypass em qualquer superfície de escrita de estado.
- [ ] G-008: Padronizar guardrails de supply chain em todos os repositórios (workflows/deps/paths sensíveis).
- [ ] G-015: Subir `edge-control.logline.world` como control-plane oficial no LAB 8GB em modo operacional estável.
- [ ] G-030: Alinhar contratos OpenAPI com topologia canônica (remover drift de host/porta, ex.: `edge-control` localhost `8080` vs runtime `18080`).

### 1.3 P0 - edge-control produção distribuída
- [ ] ECTRL-001: Evitar crescimento não limitado de `rate_buckets` (normalizar chave por identidade e adicionar eviction/TTL).
- [ ] ECTRL-002: Diferenciar falha de backend de idempotência vs request duplicada (não retornar `409 duplicate_request` em erro de storage/RPC).
- [ ] ECTRL-003: Introduzir cache de JWKS + timeout explícito no client HTTP para validação JWT (reduzir dependência de rede por request).

### 1.4 P1 - Fuel econômico e homeostase (backend)
Sem pendências abertas nesta frente.

### 1.5 P1 - LLM Gateway (pendências reais)
- [x] LLM-012: Rodar smoke autenticado em CI cobrindo fluxos JWT e compat mode até sunset completo.

### 1.6 P1 - Logic / CLI (pendências reais)
- [ ] LOGIC-013: Estender o relatório operacional consolidado com round-trip por intenção quando `OBS-001` estiver disponível.

### 1.7 P1 - Obs API (backend, sem UI nova)
- [ ] OBS-001: Expor backend de round-trip por intenção (`intake`, `linear`, `ci`, `pr`, `merge`, `deploy`) para consumo futuro da UI.
- [ ] OBS-002: Fechar alertas de `stuck job`, `stale intention` e `sync failure` com ack/resolution auditável.
- [ ] OBS-003: Harden `cli/auth/challenge*` para não expor `session_token`/dados sensíveis sem auth e sanitizar payload de status.
- [ ] OBS-004: Validar fluxo de aprovação de challenge (`pending`, `expires_at`, single-use, replay protection) antes de emitir sessão.
- [ ] OBS-005: Exigir membership tenant/app em `/api/v1/apps/:appId/keys/user` (read/write) para bloquear escrita cross-tenant por usuário autenticado.
- [ ] OBS-006: Revisar `POST /api/v1/auth/tenant/resolve` para evitar enumeração de tenant (auth obrigatória ou resposta minimizada/rate-limited).
- [ ] OBS-007: Endurecer criação de challenge CLI (`expires_at` server-side com clamp + rate limit/abuse controls + cleanup de expirados).

### 1.8 P2 - Onboarding ecossistema (depois do core estável)
- [ ] VVC-001: Publicar intentions + sync + linkage no fluxo padrão (`voulezvous-tv-codex`).
- [ ] VVP-001: Publicar intentions + CI/deploy sync + gates substantial (`VoulezvousPlataforma`).
- [ ] ONB-001: Padronizar template único de onboarding para qualquer novo app entrar no ciclo Code247/Linear/Fuel.

### 1.9 P3 - UI por último
- [ ] UI-001: Implementar toggle `Realtime | Estatística` consumindo apenas endpoints reais (`/api/v1/fuel/dashboard`, `/api/v1/fuel/reconciliation`).
- [ ] UI-002: Implementar visual de round-trip por intenção consumindo backend `OBS-001`.
- [ ] UI-003: Remover 100% dos mocks e bloquear PR de UI que introduza dado simulado fora de story/test.

## 2) Paralelização Organizada (3 agentes, 1 documento)

### 2.1 Regras únicas de execução
- [ ] PX-001: Cada agente usa branch `codex/<agent-id>-<topic>` sem editar área de ownership dos outros.
- [ ] PX-002: Contratos (`contracts/*`, `openapi/*`, `policy/*`) só mudam com revalidação global (`./scripts/validate-contracts.sh`).
- [ ] PX-003: Nenhum merge sem teste local + evidência anexada no PR (`comando`, `resultado`, `impacto`).
- [ ] PX-004: Integração em 2 janelas fixas por dia (meio do dia e fim do dia), sem rebase destrutivo.

### 2.2 Agent A - Code247 + Edge Governance Hardening
**Escopo:** `code247.logline.world/*`, `edge-control.logline.world/*` (+ ajustes mínimos em contrato quando necessário)  
**Objetivo:** tornar Code247 e o control-plane aptos para execução contínua com segurança operacional.

- [x] A-001: `C247-RDY-001` JWT+scope completo nas rotas críticas.
- [ ] A-005: `C247-RDY-007/008` merge queue + security scan obrigatório para repos críticos.
- [x] A-007: `C247-RDY-012` timeline operacional do `Code247` validada no obs-api (telemetria de etapa exposta em `/api/v1/code247/stage-telemetry`).

**DoD Agent A:** estado, merge policy e control-plane protegidos por validação central + testes verdes + logs auditáveis.

### 2.3 Agent B - Fuel + Obs Economics
**Escopo:** `llm-gateway.logline.world/*`, `logic.logline.world/supabase/migrations/*`, `obs-api.logline.world/*` para Fuel/telemetria econômica.  
**Objetivo:** fechar a camada econômica com alertas, calibração e observabilidade útil para operação.

Sem pendências abertas no escopo do Agent B.

**DoD Agent B:** alertas e calibração de Fuel operacionais, com segmentação por policy e visibilidade útil no obs-api.

### 2.4 Agent C - Logic CLI + Integration Harness
**Escopo:** `logic.logline.world/*`, `scripts/*`, `contracts/*` (somente catálogo/cookbook/harness), suporte a `obs-api` para testes de contrato.  
**Objetivo:** transformar CLI em backbone operacional e fechar test harness severo de integração.

- [ ] C-001: `G-001` round-trip único `manifest.intentions -> Code247 -> Linear -> pipeline -> PR/merge -> deploy` com rastreabilidade fim-a-fim.
- [ ] C-002: `G-004` auth service-to-service unificada (JWT + escopo de projeto) em todos os serviços centrais.
- [ ] C-003: `G-030` alinhar contratos OpenAPI com topologia canônica e remover drift de host/porta.
- [ ] C-004: `OBS-001` backend de round-trip por intenção para consumo futuro da UI.
- [x] C-006: `LLM-012` smoke autenticado do gateway cobrindo JWT e compat mode até sunset.

**DoD Agent C:** execução de ponta a ponta acionável por um comando, com relatório auditável.

## 3) Testes Severos de Integração (pendências novas)

### 3.1 Novos cenários obrigatórios
- [ ] TST-011: validar `edge-control` com JWKS real + idempotência persistente em cenário restart/multi-instance.
- [ ] TST-012: falha/intermitência de Linear/GitHub preserva timeline, fila assíncrona e sinais operacionais do Code247.
- [ ] TST-013: `policy_version` segmentada por tenant/app não mistura cálculo de Fuel entre tenants.
- [ ] TST-014: validar security regressions do `obs-api` (`cli challenge` sem leakage/replay e `user keys` com membership enforcement estrito).
- [ ] TST-015: validar que `tenant/resolve` não permite enumeração indevida e que criação de challenge aplica TTL/rate-limit server-side.

### 3.2 Gate incremental
- [ ] TST-GATE-004: qualquer mudança em `Code247 timeline`, `edge-control auth/idempotency` ou `Fuel policy_version` deve adicionar teste severo correspondente.

Status atual (2026-03-06): `./scripts/integration-severe.sh` segue verde no baseline atual e novos cenários acima passam a ser obrigatórios para o próximo ciclo.

## 4) Definition of Done (ciclo atual)
- [ ] DOD-001: Code247 apto a receber tasks contínuas sem intervenção manual fora dos checkpoints definidos.
- [ ] DOD-002: auth service-to-service unificada em todos os serviços centrais.
- [ ] DOD-004: suíte severa de integração rodando em CI e bloqueando regressão.
- [ ] DOD-005: UI inicia apenas após backend contracts/gates/testes estarem estáveis.

---

## Milestones Alcançados e Principais Entregas

### Infra e runtime
- [x] Topologia canônica unificada (`service_topology.json`) gerando PM2 + cloudflared.
- [x] Stack central estável em build/start (sem dev server em produção no `obs-api`).
- [x] Smoke canônico (`scripts/smoke.sh`) adotado como verificação operacional.

### Governança e contratos
- [x] Envelopes padrão (`request_id`, `output_schema`, erro canônico) aplicados nos serviços centrais.
- [x] OpenAPI publicado/validado nos serviços principais e validador global ativo.
- [x] Contratos canônicos publicados: events registry, policy-set, schemas e OpenAPI de edge/inference.

### Testes severos e CI
- [x] Suíte `integration-severe` implementada com gate fail-closed e relatório auditável por execução.
- [x] Workflows críticos preparados para `merge_group` e `security-scan` unificado versionado no monorepo (`scripts/security-scan.sh` + artifact em CI); falta apenas aplicar o ruleset remoto no GitHub para fechar `C247-RDY-007/008`.
- [x] Cenários críticos cobrindo E2E feliz, bloqueio de `Done` sem evidência, replay/idempotência, fallback LLM, `red-main`, flaky CI, drift Fuel, auth inválido e caos leve de rede.
- [x] Gate de CI exigindo atualização de testes severos quando estado/contrato/fuel são alterados.

### Code247 + Linear
- [x] Intake `/intentions`, sync `/intentions/sync`, auto-claim e workflow Linear oficial integrados.
- [x] Gate de evidência para avanço de estado implementado (incluindo bloqueio de transições inválidas relevantes).
- [x] Webhook hardening (assinatura, anti-replay, idempotência, DLQ/retry) implementado.
- [x] Allowlist de comandos do runner por repo enforçada em modo fail-closed.
- [x] Evidência obrigatória (`plan + AC + risco + backout + evidências`) exigida antes de PR mergeável.
- [x] Validador canônico de transição/evidência consolidado e reutilizado em intake, sync, polling e webhook path.
- [x] `code247_run_timeline` e outbox assíncrona de ações do Linear com retry/DLQ habilitados para troubleshooting operacional.
- [x] Emissão de `REVIEW_LOOP_EXHAUSTED` e governança de plano configurável por policy ativadas no pipeline.
- [x] Merge policy `light/substantial` com override auditável explícito e trilha de decisão persistida no pipeline/evidence store.
- [x] Lease/deadline/heartbeat por etapa com sweeper horizontal de expiração e auto-escalonamento fail-closed para jobs presos.
- [x] Claim atômico de `PENDING -> PLANNING` com lease no storage, eliminando janela de double execution entre instâncias.
- [x] Smoke oficial `scripts/smoke-code247-stage-lease.sh` adicionando prova runtime de expiração de lease com evento único e sem poluir Linear.
- [x] Resilience layer compartilhada com retries/backoff + circuit breaker aplicada às integrações críticas de `llm-gateway`, `Linear OAuth/GraphQL` e `GitHub PR/status/merge`.
- [x] Testes HTTP integrados de governança de estado (`/intentions` + `/intentions/sync`) cobrindo `In Progress -> Ready for Release`, bloqueio de `In Progress -> Done` e `Ready for Release -> Done`, com gate oficial em CI.
- [x] Telemetria de latência/custo por etapa do `Code247` exposta no `obs-api` em `/api/v1/code247/stage-telemetry`, com smoke autenticado cobrindo acesso via JWT.
- [x] Timeline operacional do `Code247` exposta no `obs-api` em `/api/v1/code247/run-timeline`, combinando `jobs`, `checkpoints` e `code247_events` com validação via smoke autenticado.

### edge-control
- [x] Serviço de control-plane funcional com auth, request id, rate-limit e middleware de idempotência.
- [x] Endpoints `/v1/intention/draft`, `/v1/pr/risk`, `/v1/fuel/diff/route` publicados com envelopes canônicos.
- [x] Policy engine com `policy-set.v1.1.json` e emissão de `GateDecision.v1` integrado ao fluxo.
- [x] JWT via JWKS real, contract tests dos handlers críticos e resilience layer downstream habilitados.
- [x] Idempotência persistida em backend compartilhado via Supabase RPC, com fallback SQLite apenas para dev/teste local.
- [x] Orquestração `edge-control -> Code247` migrou do bearer estático para JWT service-to-service curto com `scope` + `code247_projects`, mantendo `CODE247_INTENTIONS_TOKEN` apenas como fallback legado.

### LLM Gateway
- [x] Redução adicional de latência local com alvo operacional por modo.
- [x] Envelope padrão aplicado nos endpoints JSON e erros canônicos.
- [x] Compatibilidade controlada entre API key legado e JWT Supabase com trilha de sunset.
- [x] Smoke autenticado do gateway versionado e plugado em CI, cobrindo JWT service token, compat mode legado e bloqueio explícito quando legacy está `disabled`.
- [x] Persistência normalizada em `llm_requests` com provider/model/mode.
- [x] Contrato formal gateway -> Code247 publicado.
- [x] Fuel Supabase tornado fonte primária, sem dependência operacional de SQLite local.
- [x] Publicação de `manifest.intentions.json` pós-release integrada ao fluxo oficial.
- [x] Métricas operacionais por modo/provider, incluindo fallback, timeout e custo efetivo, expostas via `obs-api`.

### Fuel e observabilidade
- [x] Camada de valuation + points (`fuel_valuations`, `fuel_points_v1`) e guardrails de metadata em produção.
- [x] Baseline sazonal e dashboard backend Fuel (`/api/v1/fuel/dashboard`) ativos.
- [x] Backend de reconciliação Fuel (`/api/v1/fuel/reconciliation`) ativo com segmentação por app/source/provider/model.
- [x] Reconciliação cloud real (`usd_settled`) via providers com idempotência e retries.
- [x] Medição de energia local com `confidence` explícita e método auditável.
- [x] Alertas Fuel, jobs recorrentes de baseline/ops, calibração dos `k_*` e segmentação de `policy_version` por tenant/app publicados no backend.
- [x] Dashboard/reconciliation com cortes por `policy_version` e `precision_level` habilitados para análise operacional.
- [x] Smoke autenticado do `obs-api` validando `dashboard`, `alerts`, `calibration`, `reconciliation`, `ops` e materialização remota no Supabase, com gate reutilizável em CI via Doppler.
- [x] Objetivo de Fuel do ciclo atual (`DOD-003`) alcançado: alertas, calibração e segmentação observáveis por tenant/app.

### CLI e catálogo canônico
- [x] `logline catalog export` implementado.
- [x] Catálogo unificado CLI+API gerado em `contracts/generated/capability-catalog.v1.json`.
- [x] Endpoint de catálogo no `obs-api` (`/api/v1/capabilities/catalog`) publicado para abastecer cookbook/UI.
- [x] Entry point oficial `./scripts/verify-operations.sh` criado para `smoke + sync-canon check + cookbook + contracts + integration-severe`.
- [x] Relatório operacional consolidado com artefatos e caminhos de rastreabilidade publicado em `artifacts/operations-verify-report.{json,md}`.
- [x] Fluxo CLI/runtime padronizado para publicar intentions, sync status e replay.
- [x] Contratos policy/gates consumíveis pelo Code247 sem adapter ad-hoc.
- [x] Outputs de execução estabilizados para auditoria/replay.
- [x] Publicação de `manifest.intentions.json` + linkage `.code247/linear-meta.json` em fluxo padrão de release.
- [x] Cookbook gerado automaticamente a partir de catálogo canônico.
- [x] Hardening de auth policies do workspace e sync Canon AST -> schemas/TypeScript sem drift.
