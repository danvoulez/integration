# LogLine Ecosystem Integration Blueprint - Addendum v1.1

Version: 1.1.0
Date: 2026-03-05
Status: ACTIVE ADDENDUM (extends v1.0.0)
Owner: UBL Founder
Parent: `INTEGRATION_BLUEPRINT.md`

---

## 0) Purpose

This addendum keeps the current canonical blueprint and introduces an operational pivot:

1. Fuel is upgraded from billing-only to causal operational ledger.
2. Transistor pattern becomes mandatory for LLM-driven decisions.
3. Human interaction is restricted to two explicit YES/HUMAN checkpoints.
4. Fuel semantic imaging becomes a recurring, reproducible process.

This is a controlled evolution, not a reset.

---

## 1) Mandatory Changes

### 1.1 Fuel-first operational model
- `fuel_events` remains append-only and idempotent.
- `metadata.event_type` becomes the semantic event contract.
- `trace_id` and `parent_event_id` are required for causal chains.
- Vectors/snapshots/diffs are derived indexes, never source-of-truth.

### 1.2 Transistor pattern
- LLM emits `OpinionSignal.v1` only.
- Rust judge applies deterministic rules and emits `GateDecision.v1`.
- No direct execution from LLM output is allowed.

### 1.3 Official human checkpoints
- YES/HUMAN #1: confirm `DraftIntention` before execution.
- YES/HUMAN #2: approve rollback/undo/destructive reversal.

### 1.4 New control-plane service
- Add `edge-control.logline.world` (Rust).
- Scope: auth, policy gates, orchestration, webhooks, scheduler, fuel emission.

---

## 2) Architecture (v1.1)

### 2.1 Ledger Plane
- Supabase Postgres and Realtime.
- Canonical tables include: `fuel_events`, `llm_requests`, `code247_jobs`, `code247_checkpoints`.
- Semantic derivatives: `fuel_semantic_docs`, `fuel_snapshots`, `fuel_diffs`.

### 2.2 Control Plane
- Runs on LAB 8GB.
- Executes deterministic policy and idempotent workflows.
- Integrates Code247, Linear, GitHub, llm-gateway, inference plane.

### 2.3 Inference Plane
- Runs on LAB 512.
- Exposes JSON-only APIs for drafting/classification/embedding.
- Isolated from public internet; only edge-control can reach it.

---

## 3) Runtime Topology

### 3.1 LAB 256 (Operator/UI)
- `obs-api.logline.world` operator console.
- Human confirms intention and triggers rollback.

### 3.2 LAB 8GB (Edge/Server)
- `edge-control` + cloudflared + PM2.
- Webhooks, scheduler, orchestration, policy judge.

### 3.3 LAB 512 (Inference)
- Intention drafting, PR risk opinion, fuel diff routing, embeddings.
- Private network allowlist from LAB 8GB only.

---

## 4) Canonical Contracts Added by v1.1

- `contracts/events.registry.json`
- `policy/policy-set.v1.1.json`
- `contracts/schemas/opinion-signal.v1.schema.json`
- `contracts/schemas/gate-decision.v1.schema.json`
- `contracts/schemas/draft-intention.v1.schema.json`
- `contracts/schemas/rollback-plan.v1.schema.json`
- `contracts/openapi/edge-control.v1.openapi.yaml`
- `contracts/openapi/inference-plane.v1.openapi.yaml`

All services must validate against these contracts in CI.

---

## 5) Official End-to-End Flow

1. Intake receives intention (`POST /intentions` or equivalent source).
2. Intention is normalized and synced to Linear.
3. Inference drafts structured intention.
4. Human confirms draft (YES/HUMAN #1).
5. Code247 executes: plan -> code -> self-review -> PR -> CI -> merge/deploy.
6. Deterministic gates control auto-merge versus auto-block/cloud escalation (no human merge stage).
7. Linear status updates to `Done` only with CI/deploy evidence.
8. Rollback path is always available via YES/HUMAN #2.

---

## 6) Migration Plan and DoD

### Phase 1: canonical alignment
- llm-gateway: JWT + Supabase persistence + telemetry.
- code247: JWT + llm-gateway-only calls + Supabase jobs/checkpoints.
- obs-api: refurbish completion and Supabase-only read model.
- DoD: no critical path depends on local SQLite.

### Phase 2: causal ledger + transistor
- Enforce event registry and reason code taxonomy.
- Emit `*.opinion_emitted` and `*.gate_decided` consistently.
- DoD: every major decision is reconstructable from Fuel trace.

### Phase 3: intention drafting productized
- Deploy draft/confirm workflow in HQ.
- DoD: operator can go from free-text to confirmed intention with evidence.

### Phase 4: rollback productized
- Require rollback plan in substantial changes and enforce rollback-ready evidence before merge.
- DoD: rollback is deterministic, fast, and auditable.

### Phase 5: semantic imaging recurring loop
- Enable hourly/daily snapshots and diff routing.
- DoD: recurring suggestions are reproducible and evidence-backed.

---

## 7) Governance Rules

- No direct push to `main` in critical repos.
- No `Done` in Linear without evidence links.
- Sensitive paths always require cloud re-evaluation + stricter automated gates (never direct light auto-merge).
- Unknown or malformed LLM output fails closed.
- Idempotency keys are mandatory for all webhook and intake processing.
