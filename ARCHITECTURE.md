# LogLine Ecosystem Architecture

**Version:** 2.0.0  
**Date:** 2026-03-06  
**Status:** Canonical Source of Truth  
**Supersedes:** INTEGRATION_BLUEPRINT.md, INTEGRATION_BLUEPRINT_ADDENDUM_v1.1.md

---

## 1) Purpose

This document defines the complete architecture for the LogLine ecosystem.
It is the **single source of truth** for how services connect, authenticate, communicate, and bill.

All integration work MUST reference this document.
Deviations require explicit approval and amendment.

---

## 2) Ecosystem Overview

### 2.1 Service Inventory

| Service | Domain | Type | Port | Status |
|---------|--------|------|------|--------|
| `logic.logline.world` | CLI + Core Crates (HQ) | Rust workspace | N/A | Production |
| `llm-gateway.logline.world` | LLM Routing + Billing | Rust binary | 7700 | Production |
| `code247.logline.world` | Autonomous Coding Agent | Rust binary | 4001 | Production |
| `edge-control.logline.world` | Control Plane (policy, orchestration) | Rust binary | 8080 | Development |
| `obs-api.logline.world` | Observability Dashboard | Next.js | 3001 | Production |

### 2.2 Infrastructure Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Auth | Supabase Auth + JWKS | Identity, JWT issuance, tenant resolution |
| Database | Supabase Postgres | Canonical state, RLS enforcement |
| Realtime | Supabase Realtime | Job status broadcast, live updates |
| Storage | Supabase Storage | Evidence artifacts, logs |
| Process | PM2 | Service lifecycle management |
| Tunnel | Cloudflare Tunnel | Public DNS → local services |
| DNS | Cloudflare | `*.logline.world` routing |

### 2.3 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              CLOUDFLARE                                         │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │  DNS: *.logline.world                                                     │  │
│  │  ├── llm-gateway.logline.world ──► tunnel ──► localhost:7700              │  │
│  │  ├── obs-api.logline.world ──────► tunnel ──► localhost:3001              │  │
│  │  ├── code247.logline.world ──────► tunnel ──► localhost:4001              │  │
│  │  ├── edge-control.logline.world ─► tunnel ──► localhost:8080              │  │
│  │  └── logic.logline.world ────────► (CLI binary, no tunnel)                │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────────┘
                                         │
┌────────────────────────────────────────▼────────────────────────────────────────┐
│                              LOCAL HOST (PM2)                                   │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                │
│  │ llm-gateway │ │   code247   │ │edge-control │ │   obs-api   │                │
│  │   :7700     │ │   :4001     │ │   :8080     │ │   :3001     │                │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘                │
│         └───────────────┴───────────────┴───────────────┘                       │
│                                   │                                             │
│                    ┌──────────────▼──────────────┐                              │
│                    │      Ollama Instances       │                              │
│                    │      LAB-256 / LAB-512      │                              │
│                    └─────────────────────────────┘                              │
└─────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SUPABASE PROJECT                                   │
│  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐        │
│  │     AUTH      │ │   POSTGRES    │ │   REALTIME    │ │   STORAGE     │        │
│  │   • JWKS      │ │   • fuel_*    │ │   • channels  │ │   • evidence  │        │
│  │   • sessions  │ │   • code247_* │ │   • broadcast │ │   • logs      │        │
│  │   • hooks     │ │   • llm_*     │ │               │ │   • exports   │        │
│  └───────────────┘ └───────────────┘ └───────────────┘ └───────────────┘        │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Runtime Topology

| Host | Services | Purpose |
|------|----------|---------|
| LAB-256 | obs-api, llm-gateway, code247 | Operator console, main services |
| LAB-8GB | edge-control, cloudflared | Control plane, webhooks, orchestration |
| LAB-512 | Ollama (inference) | LLM inference, private network |

---

## 3) Governance Model

### 3.1 Transistor Pattern (Mandatory)

LLM output is **never executed directly**:

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│     LLM     │ ──►  │ Rust Judge  │ ──►  │  Execution  │
│  (Opinion)  │      │   (Rules)   │      │  (Action)   │
└─────────────┘      └─────────────┘      └─────────────┘
       │                    │
       ▼                    ▼
 OpinionSignal.v1    GateDecision.v1
```

1. LLM emits `OpinionSignal.v1` — suggestion only
2. Rust judge applies deterministic rules from `policy-set.v1.1.json`
3. `GateDecision.v1` is emitted with full audit trail
4. Unknown or malformed LLM output **fails closed**

### 3.2 Human Checkpoints

Only two points require human intervention:

| Checkpoint | When | Action |
|------------|------|--------|
| **YES/HUMAN #1** | Confirm `DraftIntention` | Approve before Code247 execution |
| **YES/HUMAN #2** | Approve rollback | Confirm destructive reversal |

All other operations are automated with deterministic gates.

### 3.3 Authority Hierarchy

```
1. Rust services      → Business logic authority
2. Supabase           → State/persistence authority  
3. fuel_events        → Billing/usage authority
4. Linear             → Human interface (not source of truth)
5. LLM                → Opinion emitter only (never decides)
```

---

## 4) Service Boundaries

### 4.1 logic.logline.world (HQ)

**Role:** Central authority for business logic, identity, and policy.

**Owns:**
- Rust crate definitions (`logline-*`)
- CLI as first implementation surface
- Supabase migrations (canonical schema)
- Fuel ledger schema
- RBAC/RLS policies

**Does NOT:** Run as a persistent server.

### 4.2 llm-gateway.logline.world

**Role:** Unified LLM routing (local + cloud providers).

**Owns:**
- Model routing logic
- Provider failover/circuit breaker
- Token counting and fuel emission
- Per-client usage tracking

**Consumes:** `logline-auth` (JWT), `logline-supabase` (fuel)

### 4.3 code247.logline.world

**Role:** Autonomous coding agent (plan → code → review → PR → CI → merge).

**Owns:**
- Job lifecycle and state machine
- Linear integration
- Git operations
- Evidence collection

**Consumes:** `llm-gateway` (LLM), `logline-auth` (JWT), `logline-supabase` (persistence)

### 4.4 edge-control.logline.world

**Role:** Control plane for policy enforcement and orchestration.

**Owns:**
- Policy gates (`policy-set.v1.1.json`)
- Transistor pattern enforcement
- Webhook handling (Linear, GitHub)
- Scheduler and orchestration
- Human checkpoint gates

**Consumes:** All other services via authenticated APIs

### 4.5 obs-api.logline.world

**Role:** Observability dashboard (observe only, never decide).

**Owns:**
- Event ingestion from all services
- Timeline and trace visualization
- Fuel dashboards
- Operator alerts

**Does NOT:** Implement business logic or make decisions.

---

## 5) Identity and Auth

### 5.1 Canonical Identity Scope

Every protected operation MUST resolve:

```json
{
  "tenant_id": "workspace/organization",
  "app_id": "service identifier",
  "user_id": "actor identity"
}
```

### 5.2 Auth by Service

| Service | Method |
|---------|--------|
| CLI | Supabase JWT + Touch ID session |
| obs-api | Supabase JWT |
| llm-gateway | Supabase JWT OR API key (compat mode) |
| code247 | Supabase JWT (service account) |
| edge-control | Supabase JWT (service account) |

### 5.3 JWT Claims Contract

```json
{
  "sub": "user-uuid",
  "email": "user@example.com",
  "role": "authenticated",
  "workspace_id": "tenant-id",
  "app_id": "app-id",
  "iss": "https://<project>.supabase.co/auth/v1",
  "exp": 1709424000
}
```

### 5.4 Service-to-Service Auth

For backend services operating without user context:
- **Service Account JWT**: Long-lived JWT with `app_id` claim, no `user_id`
- Cross-tenant access is **denied by default**

---

## 6) Data Model

### 6.1 Fuel System (3-Layer Model)

See [FUEL_SYSTEM_SPEC.md](FUEL_SYSTEM_SPEC.md) for complete specification.

| Layer | Table | Purpose |
|-------|-------|---------|
| A | `fuel_events` | Append-only measurement facts |
| B | `fuel_valuations` | USD/energy/carbon values (upsertable) |
| C | `fuel_points` | Control currency (view/derived) |

**Invariants:**
- `fuel_events` is append-only (no UPDATE, no DELETE)
- Every billable action emits exactly one fuel event
- `idempotency_key` prevents duplicate charges

### 6.2 LLM Requests

**Table:** `llm_requests`

Tracks individual LLM API calls for observability.
Links to `fuel_events` via `fuel_event_id`.

### 6.3 Code247 State

**Tables:** `code247_jobs`, `code247_events`, `code247_checkpoints`

**Key invariants:**
- Jobs follow state machine: `Ready → In Progress → Ready for Release → Done`
- No `Done` without evidence (CI URL, deploy ID)
- No bypass of state transitions

### 6.4 Events Registry

All event types defined in `contracts/events.registry.json`:
- Metadata schema per event type
- Valid reason codes
- Required fields enforced at emission

---

## 7) Communication Contracts

### 7.1 LLM Gateway API

**Base:** `https://llm-gateway.logline.world`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/chat/completions` | POST | Chat generation |
| `/v1/modes` | GET | Available modes |
| `/metrics` | GET | Prometheus metrics |
| `/health` | GET | Health check |

**Headers:**
- `Authorization: Bearer <jwt>` — Required
- `X-Mode: genius|fast|code` — Optional mode override

### 7.2 Code247 API

**Base:** `https://code247.logline.world`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/intentions` | POST | Intake new intention |
| `/intentions/sync` | POST | Sync execution state |
| `/intentions/{workspace}/{project}` | GET | Get linkage |
| `/jobs` | GET | List jobs |
| `/health` | GET | Health check |

### 7.3 Edge Control API

**Base:** `https://edge-control.logline.world`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/intention/draft` | POST | Draft intention |
| `/v1/pr/risk` | POST | PR risk assessment |
| `/v1/orchestrate/intention-confirmed` | POST | YES/HUMAN #1 |
| `/v1/orchestrate/rollback` | POST | YES/HUMAN #2 |
| `/health` | GET | Health check |

### 7.4 Supabase Realtime Channels

| Channel | Purpose |
|---------|---------|
| `code247:jobs:{tenant_id}` | Job status updates |
| `fuel:events:{tenant_id}` | Fuel event stream |

---

## 8) Shared Crates

```
logline-api          ← Trait definitions, shared models
    ↑
logline-core         ← Domain policy, catalog validation
    ↑
logline-auth         ← JWT/JWKS verification
    ↑
logline-supabase     ← Supabase client (PostgREST, Realtime, Storage)
    ↑
logline-connectors   ← External service integrations
    ↑
logline-runtime      ← Runtime engine orchestration
    ↑
logline-cli          ← CLI binary
```

---

## 9) Canonical Contracts

All services validate against these contracts in CI:

| Contract | Path |
|----------|------|
| Events registry | `contracts/events.registry.json` |
| Policy set | `policy/policy-set.v1.1.json` |
| Opinion signal | `contracts/schemas/opinion-signal.v1.schema.json` |
| Gate decision | `contracts/schemas/gate-decision.v1.schema.json` |
| Draft intention | `contracts/schemas/draft-intention.v1.schema.json` |
| Rollback plan | `contracts/schemas/rollback-plan.v1.schema.json` |

---

## 10) End-to-End Flow

```
1. Intake receives intention (POST /intentions or source)
2. Intention normalized and synced to Linear
3. Inference drafts structured intention
4. Human confirms draft (YES/HUMAN #1)
5. Code247 executes: plan → code → review → PR → CI → merge
6. Deterministic gates control auto-merge vs block
7. Linear status → `Done` only with CI/deploy evidence
8. Rollback available via YES/HUMAN #2
```

---

## 11) Invariants (Non-Negotiable)

### Architecture
- `INV-001`: Business logic MUST reside in Rust crates, not UI handlers
- `INV-002`: CLI MUST be first implementation surface for new capabilities
- `INV-003`: All services MUST resolve identity scope before protected operations
- `INV-004`: Supabase is the sole persistence backend
- `INV-005`: LLM output MUST go through Transistor pattern

### Auth
- `INV-101`: All external-facing endpoints MUST validate auth
- `INV-102`: Tokens MUST NOT be logged or persisted in plaintext
- `INV-103`: Cross-tenant access denied by default

### Billing
- `INV-201`: Every billable action MUST emit a `fuel_event`
- `INV-202`: `fuel_events` table MUST be append-only
- `INV-203`: `idempotency_key` MUST prevent duplicate charges

### Operations
- `INV-301`: All services MUST run under PM2 supervision
- `INV-302`: All public DNS MUST route through Cloudflare Tunnel
- `INV-303`: Secrets MUST NOT exist in git or env files on disk
- `INV-304`: Unknown LLM output MUST fail closed

---

## 12) Related Documents

| Document | Purpose |
|----------|---------|
| [FUEL_SYSTEM_SPEC.md](FUEL_SYSTEM_SPEC.md) | Complete Fuel specification |
| [SERVICE_TOPOLOGY.md](SERVICE_TOPOLOGY.md) | Network, ports, DNS details |
| [GOVERNANCE.md](GOVERNANCE.md) | Decision-making and autonomy |
| [SECURITY.md](SECURITY.md) | Security policies and reporting |
| [INFRA_RUNBOOK.md](INFRA_RUNBOOK.md) | Operational procedures |
