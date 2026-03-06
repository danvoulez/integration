# Tasklist Geral do Ecossistema

**Updated:** 2026-03-06  
**Foco do ciclo atual:** hardening backend + fechar lacunas de integração (`code247`, `llm-gateway`, `logic/CLI`)  
**Regra do ciclo:** UI por último, depois de contratos/gates/testes severos estarem verdes.

## 1) Backlog Ativo (somente pendências)

### 1.1 P0 - Bloqueios para Code247 aceitar carga séria
- [ ] C247-RDY-001: Fechar authn/authz completo em `/intentions*`, `/jobs`, webhooks e workers com JWT + project scope (substituir token único legado).
- [ ] C247-RDY-002: Consolidar validador único de transição/evidência (sem bypass) reutilizado por intake, sync, polling e webhook path.
- [ ] C247-RDY-003: Completar merge policy `light/substantial` com override auditável explícito e trilha de decisão persistida.
- [x] C247-RDY-004: Enforçar allowlist de comandos do runner por repo (fail-closed para comando fora de policy).
- [x] C247-RDY-005: Tornar `plan + AC + risco + backout + evidências` obrigatório para toda execução antes de PR mergeável.
- [ ] C247-RDY-006: Fechar integração de testes de estado (`G-007-P5/P6`) com prova em CI (positivo + negativo + smoke E2E).
- [ ] C247-RDY-007: Ativar merge queue (`merge_group`) em repositórios críticos para evitar green-on-stale.
- [ ] C247-RDY-008: Tornar security scan obrigatório para repos críticos antes de merge substantial.
- [ ] C247-RDY-009: Implementar retries/backoff + circuit breaker para Linear/GitHub/CI.
- [ ] C247-RDY-010: Implementar watchdog de stuck jobs com timeout por etapa e auto-escalonamento.
- [ ] C247-RDY-011: Expor telemetria de latência/custo por etapa (`plan/code/review/ci/merge/deploy`) no obs-api.

### 1.2 P0 - Hardening central (cross-app)
- [ ] G-001: Fechar round-trip único `manifest.intentions -> Code247 -> Linear -> pipeline -> PR/merge -> deploy` com rastreabilidade fim-a-fim.
- [ ] G-004: Padronizar auth service-to-service única (JWT + escopo de projeto) em todos os serviços centrais.
- [ ] G-007: Fechar enforcement completo de `Done` sem bypass em qualquer superfície de escrita de estado.
- [ ] G-008: Padronizar guardrails de supply chain em todos os repositórios (workflows/deps/paths sensíveis).
- [ ] G-015: Subir `edge-control.logline.world` como control-plane oficial no LAB 8GB em modo operacional estável.
- [ ] G-030: Alinhar contratos OpenAPI com topologia canônica (remover drift de host/porta, ex.: `edge-control` localhost `8080` vs runtime `18080`).

### 1.3 P1 - Fuel econômico e homeostase (backend)
- [x] G-021: Integrar reconciliação cloud real (`usd_settled`) via APIs dos providers (OpenAI/Anthropic) com idempotência e retries.
- [x] G-022: Implementar medição de energia local (Qwen2/Ollama) com `confidence` explícita e método auditável.
- [ ] G-024: Fechar alertas Fuel no `obs-api` (pressão latência/erro/energia, drift e cobertura L0-L3 com thresholds operacionais).
- [ ] G-024-C: Materializar jobs recorrentes de baseline/alerts com evidência operacional diária.

### 1.4 P1 - LLM Gateway (pendências reais)
- [x] LLM-001: Redução adicional de latência local (cache/roteamento/warm state) com alvo p95 definido por modo.
- [x] LLM-005: Garantir envelope padrão em todos endpoints JSON (não só chat), incluindo erros canônicos.
- [x] LLM-006: Unificar API key legado com JWT Supabase sem ambiguidade operacional (modo compat controlado + sunset).
- [x] LLM-007: Persistir requests normalizadas em `llm_requests` (request_id + plan_id + provider/model/mode).
- [x] LLM-008: Publicar contrato formal gateway->code247 (incluindo campos de CI target e fallback behavior).
- [x] LLM-009: Emitir fuel events para Supabase como fonte primária e remover dependência operacional de SQLite local.
- [x] LLM-010: Publicar `manifest.intentions.json` após cada release do gateway para manter sincronização oficial com Linear/Code247.

### 1.5 P1 - Logic / CLI (pendências reais)
- [x] LOGIC-001: Consolidar `CLI + runtime` para publicar intentions e sync status automaticamente.
- [x] LOGIC-002: Garantir contratos policy/gates consumíveis pelo Code247 sem adapter ad-hoc.
- [x] LOGIC-003: Padronizar outputs de execução para auditoria/replay (shape estável).
- [x] LOGIC-007: Publicar e manter `manifest.intentions.json` + linkage `.code247/linear-meta.json` em fluxo padrão de release.
- [x] LOGIC-008: Garantir geração de cookbook a partir de catálogo canônico em pipeline (não manual).
- [x] LOGIC-009: Harden auth policies do workspace (SEC/HARD items) e garantir publicação de atualizações de schema via `logline-supabase`.
- [x] LOGIC-010: Manter `workspace.manifest` schemas sincronizados com Canon AST e geração TypeScript derivada do Rust (sem drift).

### 1.6 P1 - Obs API (backend, sem UI nova)
- [ ] OBS-001: Expor backend de round-trip por intenção (`intake`, `linear`, `ci`, `pr`, `merge`, `deploy`) para consumo futuro da UI.
- [ ] OBS-002: Fechar alertas de `stuck job`, `stale intention` e `sync failure` com ack/resolution auditável.
- [ ] OBS-014: Cobrir endpoints novos com testes de contrato e smoke autenticado em CI.

### 1.7 P2 - Onboarding ecossistema (depois do core estável)
- [ ] VVC-001: Publicar intentions + sync + linkage no fluxo padrão (`voulezvous-tv-codex`).
- [ ] VVP-001: Publicar intentions + CI/deploy sync + gates substantial (`VoulezvousPlataforma`).
- [ ] ONB-001: Padronizar template único de onboarding para qualquer novo app entrar no ciclo Code247/Linear/Fuel.

### 1.8 P3 - UI por último
- [ ] UI-001: Implementar toggle `Realtime | Estatística` consumindo apenas endpoints reais (`/api/v1/fuel/dashboard`, `/api/v1/fuel/reconciliation`).
- [ ] UI-002: Implementar visual de round-trip por intenção consumindo backend `OBS-001`.
- [ ] UI-003: Remover 100% dos mocks e bloquear PR de UI que introduza dado simulado fora de story/test.

## 2) Paralelização Organizada (3 agentes, 1 documento)

### 2.1 Regras únicas de execução
- [ ] PX-001: Cada agente usa branch `codex/<agent-id>-<topic>` sem editar área de ownership dos outros.
- [ ] PX-002: Contratos (`contracts/*`, `openapi/*`, `policy/*`) só mudam com revalidação global (`./scripts/validate-contracts.sh`).
- [ ] PX-003: Nenhum merge sem teste local + evidência anexada no PR (`comando`, `resultado`, `impacto`).
- [ ] PX-004: Integração em 2 janelas fixas por dia (meio do dia e fim do dia), sem rebase destrutivo.

### 2.2 Agent A - Code247 Governance Hardening
**Escopo:** `code247.logline.world/*` (+ ajustes mínimos em contrato quando necessário)  
**Objetivo:** tornar Code247 apto para execução contínua com segurança operacional.

- [ ] A-001: `C247-RDY-001` JWT+scope completo nas rotas críticas.
- [ ] A-002: `C247-RDY-002` validador único de transição/evidência sem bypass.
- [x] A-003: `C247-RDY-004` allowlist de comandos runner (fail-closed).
- [x] A-004: `C247-RDY-005` evidência obrigatória completa.
- [ ] A-005: testes integração de estado (`Ready -> In Progress -> Ready for Release -> Done`) com negativos.
- [ ] A-006: smoke E2E com Linear sandbox + payloads inválidos (prova de bloqueio).
- [ ] A-007: `C247-RDY-007/008` merge queue + security scan obrigatório para repos críticos.
- [ ] A-008: `C247-RDY-009/010` retries/backoff/circuit breaker + watchdog de stuck jobs.
- [ ] A-009: `C247-RDY-011` telemetria de etapa emitida e validada no obs-api.

**DoD Agent A:** estado e merge policy protegidos por validação central + testes verdes + logs auditáveis.

### 2.3 Agent B - LLM Gateway + Fuel Settlement
**Escopo:** `llm-gateway.logline.world/*`, `logic.logline.world/supabase/migrations/*` (fuel settlement), integração com obs-api ingest.  
**Objetivo:** reduzir latência local e tornar custo/settlement confiável.

- [x] B-001: `LLM-001` otimização extra de p95/p99 local com métricas antes/depois.
- [x] B-002: `LLM-005` envelope padrão em todos endpoints JSON.
- [x] B-003: `LLM-007` persistência completa em `llm_requests`.
- [x] B-004: `G-021` reconciler cloud provider (OpenAI/Anthropic) com idempotência/backoff.
- [x] B-005: `G-022` energia local + confidence em valuation.
- [x] B-006: testes de carga curta (burst) e testes de degradação (provider timeout/fallback).
- [x] B-007: `LLM-009` Fuel Supabase como fonte primária (retirar dependência operacional de SQLite).
- [x] B-008: `LLM-010` intention manifest pós-release integrado ao fluxo oficial.

**DoD Agent B:** latência local melhorada com prova, reconciliação cloud funcionando, telemetria econômica confiável.

### 2.4 Agent C - Logic CLI + Integration Harness
**Escopo:** `logic.logline.world/*`, `scripts/*`, `contracts/*` (somente catálogo/cookbook/harness), suporte a `obs-api` para testes de contrato.  
**Objetivo:** transformar CLI em backbone operacional e fechar test harness severo de integração.

- [x] C-001: `LOGIC-001/002/003` fluxo CLI/runtime padronizado para intentions/sync/replay.
- [x] C-002: `LOGIC-008` cookbook gerado automaticamente a partir de catálogo canônico.
- [x] C-003: criar suíte `integration-severe` com cenários multi-serviço (ver seção 3).
- [x] C-004: automatizar `smoke + contracts + integration-severe` em pipeline único.
- [x] C-005: gerar relatório final por execução (`request_id`, traces críticos, falhas e regressões).
- [x] C-006: `LOGIC-009/010` hardening de auth policy + sync Canon AST -> schemas/TypeScript sem drift.

**DoD Agent C:** execução de ponta a ponta acionável por um comando, com relatório auditável.

## 3) Testes Severos de Integração (obrigatórios)

### 3.1 Matriz de cenários críticos
- [x] TST-001: E2E feliz completo (`intentions -> Linear -> Code247 -> PR -> merge -> sync -> state final`).
- [x] TST-002: Tentativa de `Done` sem evidência (deve bloquear com erro determinístico e evento auditável).
- [x] TST-003: Webhook duplicado/replay (`Linear-Delivery`) não pode duplicar job/ação.
- [x] TST-004: Falha de provider LLM local e fallback controlado sem quebrar tracing.
- [x] TST-005: `red-main` ativo bloqueia novas execuções até recuperação.
- [x] TST-006: `flaky check` (falha+rerun+falha) marca `blocked: ci` corretamente.
- [x] TST-007: drift Fuel anômalo gera alerta e não altera estado sem gate.
- [x] TST-008: tokens/secrets inválidos nos serviços críticos falham de forma segura (sem bypass silencioso).
- [x] TST-009: reconciliação `estimated vs settled` com divergência acima de threshold gera incidente.
- [x] TST-010: caos leve de rede (timeout/intermitência) preserva idempotência e consistência final.

### 3.2 Gate de aceitação da suíte severa
- [x] TST-GATE-001: nenhum cenário crítico pode ficar sem teste automatizado.
- [x] TST-GATE-002: PR que altera estado/contrato/fuel sem atualizar testes severos deve falhar.
- [x] TST-GATE-003: relatório de execução severa anexado em todo merge do eixo central.

Status execução Agent C (2026-03-06): `./scripts/integration-severe.sh` gera `artifacts/integration-severe-report.json` com gate fail-closed ativo e suíte severa totalmente verde (`step_failures=0`, `scenario_failures=0`).

## 4) Definition of Done (ciclo atual)
- [ ] DOD-001: Code247 apto a receber tasks contínuas sem intervenção manual fora dos checkpoints definidos.
- [ ] DOD-002: auth service-to-service unificada em todos os serviços centrais.
- [x] DOD-003: reconciliação Fuel cloud + energia local habilitadas com precisão explícita.
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

### Code247 + Linear
- [x] Intake `/intentions`, sync `/intentions/sync`, auto-claim e workflow Linear oficial integrados.
- [x] Gate de evidência para avanço de estado implementado (incluindo bloqueio de transições inválidas relevantes).
- [x] Webhook hardening (assinatura, anti-replay, idempotência, DLQ/retry) implementado.

### Fuel e observabilidade
- [x] Camada de valuation + points (`fuel_valuations`, `fuel_points_v1`) e guardrails de metadata em produção.
- [x] Baseline sazonal e dashboard backend Fuel (`/api/v1/fuel/dashboard`) ativos.
- [x] Backend de reconciliação Fuel (`/api/v1/fuel/reconciliation`) ativo com segmentação por app/source/provider/model.

### CLI e catálogo canônico
- [x] `logline catalog export` implementado.
- [x] Catálogo unificado CLI+API gerado em `contracts/generated/capability-catalog.v1.json`.
- [x] Endpoint de catálogo no `obs-api` (`/api/v1/capabilities/catalog`) publicado para abastecer cookbook/UI.
