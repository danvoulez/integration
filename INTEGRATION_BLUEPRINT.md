# LogLine Ecosystem Integration Blueprint

Version: 1.0.0
Date: 2026-03-02
Status: **CANONICAL SOURCE OF TRUTH**
Owner: UBL Founder
Active Addendum: `INTEGRATION_BLUEPRINT_ADDENDUM_v1.1.md` (2026-03-05)

---

## 0) Purpose

This document defines the complete integration architecture for the LogLine ecosystem.
It is the single source of truth for how services connect, authenticate, communicate, and bill.

All integration work MUST reference this document.
Deviations require explicit approval and amendment to this blueprint.

---

## 1) Ecosystem Overview

### 1.1 Service Inventory

| Service | Domain | Type | Repository | Port | Status |
|---------|--------|------|------------|------|--------|
| `logic.logline.world` | CLI + Core Crates | Rust workspace | `logic.logline.world/` | N/A | Production |
| `llm-gateway.logline.world` | LLM Routing | Rust binary | `llm-gateway.logline.world/` | 7700 | Production |
| `code247.logline.world` | Autonomous Coding | Rust binary | `code247.logline.world/` | 4001 | Development |
| `obs-api.logline.world` | Dashboard/UI | Next.js | `obs-api.logline.world/` | 3001 | Production |

### 1.2 Infrastructure Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| Auth | Supabase Auth + JWKS | Identity, JWT issuance, tenant resolution |
| Database | Supabase Postgres | Canonical state, RLS enforcement |
| Realtime | Supabase Realtime | Job status broadcast, live updates |
| Storage | Supabase Storage | Evidence artifacts, logs |
| Process | PM2 | Service lifecycle management |
| Tunnel | Cloudflare Tunnel | Public DNS → local services |
| DNS | Cloudflare | `*.logline.world` routing |

### 1.3 Architecture Diagram

> **Detailed topology:** [SERVICE_TOPOLOGY.md](SERVICE_TOPOLOGY.md) contains network diagrams, port allocation, Ollama routing, and communication matrix.

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              CLOUDFLARE                                         │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │  DNS: *.logline.world                                                     │  │
│  │  ├── llm-gateway.logline.world ──► tunnel ──► localhost:7700              │  │
│  │  ├── obs-api.logline.world ──────► tunnel ──► localhost:3001              │  │
│  │  ├── code247.logline.world ──────► tunnel ──► localhost:4001              │  │
│  │  └── logic.logline.world ────────► (CLI binary, no tunnel)                │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                        │                                        │
│                             ┌──────────▼──────────┐                             │
│                             │  cloudflared tunnel │                             │
│                             │  (PM2: cloudflared) │                             │
│                             └──────────┬──────────┘                             │
└────────────────────────────────────────┼────────────────────────────────────────┘
                                         │
┌────────────────────────────────────────▼────────────────────────────────────────┐
│                              LOCAL HOST (PM2)                                   │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐                    │
│  │   llm-gateway   │ │    code247      │ │    obs-api      │                    │
│  │   (Rust)        │ │    (Rust)       │ │    (Next.js)    │                    │
│  │   :7700         │ │    :4001        │ │    :3001        │                    │
│  └────────┬────────┘ └────────┬────────┘ └────────┬────────┘                    │
│           │                   │                   │                             │
│           └───────────────────┼───────────────────┘                             │
│                               │                                                 │
│                    ┌──────────▼──────────┐                                      │
│                    │   Ollama Instances  │                                      │
│                    │   LAB-256 / LAB-512 │                                      │
│                    └─────────────────────┘                                      │
└─────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SUPABASE PROJECT                                   │
│  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐        │
│  │     AUTH      │ │   POSTGRES    │ │   REALTIME    │ │   STORAGE     │        │
│  │   • JWKS      │ │   • users     │ │   • channels  │ │   • evidence  │        │
│  │   • sessions  │ │   • tenants   │ │   • broadcast │ │   • logs      │        │
│  │   • hooks     │ │   • fuel      │ │               │ │   • exports   │        │
│  └───────────────┘ └───────────────┘ └───────────────┘ └───────────────┘        │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2) Service Boundaries

### 2.1 logic.logline.world (HQ)

**Role:** Central authority for business logic, identity, and policy.

**Owns:**
- Rust crate definitions (`logline-*`)
- Auth flow implementation (`logline-auth`)
- CLI as first implementation surface
- Supabase migrations (canonical schema)
- Fuel ledger schema
- RBAC/RLS policies

**Exposes:**
- `logline` CLI binary
- Crate APIs for other services

**Does NOT:**
- Run as a persistent server
- Accept direct HTTP traffic

### 2.2 llm-gateway.logline.world

**Role:** Unified LLM routing (local + premium providers).

**Owns:**
- Model routing logic
- Provider failover/circuit breaker
- Token counting
- Per-client usage tracking

**Consumes:**
- `logline-auth` (JWT validation)
- `logline-supabase` (fuel emission)

**Current State:**
- Auth: API key only (gateway-local)
- Storage: SQLite (`~/.llm-gateway/fuel.db`)

**Target State:**
- Auth: Supabase JWT validation via `logline-auth`
- Storage: Supabase Postgres (`fuel_events`, `llm_requests`)

### 2.3 code247.logline.world

**Role:** Autonomous coding agent (plan → code → review → commit).

**Owns:**
- Job lifecycle (pending → done)
- State machine transitions
- Git operations
- Linear integration

**Consumes:**
- `llm-gateway` for LLM calls
- `logline-auth` (JWT validation)
- `logline-supabase` (job persistence, fuel)

**Current State:**
- Auth: none
- LLM: direct Anthropic/Ollama calls (bypasses gateway)
- Storage: SQLite local (`dual_agents.db`)

**Target State:**
- Auth: Supabase JWT validation
- LLM: all calls via `llm-gateway.logline.world`
- Storage: Supabase Postgres (`code247_jobs`, `code247_checkpoints`)
- Broadcast: Supabase Realtime for job status

### 2.4 obs-api.logline.world

**Role:** Observability dashboard and API surface.

**Owns:**
- Panel/component UI
- Settings cascade
- Chat persistence
- LLM gateway proxy

**Consumes:**
- Supabase Auth (JWT mode)
- Supabase Postgres (Drizzle ORM)
- `llm-gateway` via proxy route

**Current State:** Production-ready, fully integrated with Supabase.

---

## 3) Identity and Auth Contract

### 3.1 Canonical Identity Scope

Every protected operation MUST resolve:

```
{
  tenant_id: string,   // workspace/organization
  app_id: string,      // service identifier
  user_id: string      // actor identity
}
```

### 3.2 Auth Sources by Service

| Service | Current | Target |
|---------|---------|--------|
| CLI | Supabase JWT + Touch ID session | No change |
| obs-api | Supabase JWT | No change |
| llm-gateway | API key (`LLM_API_KEY`) | Supabase JWT OR service API key |
| code247 | None | Supabase JWT (service account) |

### 3.3 JWT Claims Contract

Standard Supabase JWT with custom claims injected by `app.custom_access_token` hook:

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

### 3.4 Service-to-Service Auth

For backend services (code247, llm-gateway) operating without user context:

1. **Service Account JWT**: Long-lived JWT with `app_id` claim, no `user_id`
2. **Internal API Key**: Shared secret for local-only communication

Preference: Service Account JWT when crossing network boundary.

---

## 4) Data Contracts

Data schemas are defined in dedicated specification documents. This section provides an overview.

### 4.1 Fuel Events (Billing Ledger)

**Full specification:** `FUEL_LEDGER_SPEC.md`

**Table:** `public.fuel_events`

**Key fields:**
- `event_id` — Unique identifier
- `idempotency_key` — Prevents duplicate charges
- `tenant_id`, `app_id`, `user_id` — Scope tuple
- `units`, `unit_type` — Usage measurement
- `source` — Origin subsystem

**Invariants:**
- Append-only (no UPDATE, no DELETE)
- Every billable action emits exactly one fuel event
- `idempotency_key` prevents duplicate charges

### 4.2 LLM Requests (Gateway Telemetry)

**Table:** `public.llm_requests`

Tracks individual LLM API calls for observability and debugging.
Links to `fuel_events` via `fuel_event_id` for cost attribution.

**Key fields:** `provider`, `model`, `mode`, `input_tokens`, `output_tokens`, `latency_ms`, `success`

### 4.3 Code247 Jobs

**Tables:** `public.code247_jobs`, `public.code247_checkpoints`

Tracks autonomous coding jobs and their state machine checkpoints.

**Key fields (jobs):** `issue_id`, `status` (PENDING→DONE), `payload`, `retries`, `last_error`

**Key fields (checkpoints):** `stage` (planning, coding, review, commit), `data`

**Full schemas:** `INTEGRATION_PHASES.md` § P1-T2

---

## 5) Communication Contracts

### 5.1 LLM Gateway API

Base URL: `http://localhost:7700` (internal) or `https://llm-gateway.logline.world` (external)

**Endpoints:**
- `POST /v1/chat/completions` — OpenAI-compatible chat
- `GET /v1/models` — Available models
- `GET /v1/llm/matrix` — Model capability matrix
- `GET /health` — Health check
- `GET /v1/fuel` — Usage statistics

**Headers:**
- `Authorization: Bearer <jwt_or_api_key>` — Required
- `X-Task-Hint: planning|coding|review|background` — Optional routing hint
- `X-Mode: auto|local|premium|<route-name>` — Optional mode override

### 5.2 Code247 API

Base URL: `http://localhost:4001` (internal) or `https://code247.logline.world` (external)

**Endpoints:**
- `GET /health` — Health check
- `GET /jobs` — List recent jobs
- `POST /jobs` — Create job (webhook receiver)

### 5.3 Supabase Realtime Channels

| Channel | Purpose | Payload |
|---------|---------|---------|
| `code247:jobs:{tenant_id}` | Job status updates | `{ job_id, status, progress }` |
| `llm-gateway:health` | Provider health | `{ provider, status, latency }` |

---

## 6) Shared Rust Crates

### 6.1 Crate Dependency Graph

```
logline-api          ← trait definitions, models
    ↑
logline-core         ← domain policy, catalog
    ↑
logline-auth         ← JWT/JWKS verification
    ↑
logline-supabase     ← (NEW) Supabase client wrapper
    ↑
logline-connectors   ← backend implementations
    ↑
logline-runtime      ← runtime engine
    ↑
logline-cli          ← CLI binary
```

### 6.2 logline-supabase (NEW)

Purpose: Unified Supabase client for all Rust services.

```rust
pub struct SupabaseClient {
    url: String,
    anon_key: String,
    service_key: Option<String>,
    jwt: Option<String>,
}

impl SupabaseClient {
    // Auth
    pub async fn validate_jwt(&self, token: &str) -> Result<Claims>;
    
    // Fuel
    pub async fn emit_fuel(&self, event: FuelEvent) -> Result<()>;
    pub async fn query_fuel(&self, filter: FuelFilter) -> Result<Vec<FuelEvent>>;
    
    // Realtime
    pub async fn broadcast(&self, channel: &str, event: &str, payload: Value) -> Result<()>;
    
    // Storage
    pub async fn upload(&self, bucket: &str, path: &str, data: &[u8]) -> Result<String>;
    pub async fn download(&self, bucket: &str, path: &str) -> Result<Vec<u8>>;
    
    // Postgres (via PostgREST)
    pub fn from(&self, table: &str) -> QueryBuilder;
}
```

### 6.3 Integration Points per Service

| Service | Uses Crate | Purpose |
|---------|-----------|---------|
| llm-gateway | `logline-auth` | JWT validation |
| llm-gateway | `logline-supabase` | Fuel emission, request logging |
| code247 | `logline-auth` | JWT validation |
| code247 | `logline-supabase` | Job persistence, fuel, realtime |
| CLI | `logline-auth` | JWT verification |
| CLI | `logline-supabase` | Direct Supabase operations |

---

## 7) Infrastructure Configuration

Detailed infrastructure configuration is maintained in dedicated documents:

| Topic | Document | Contents |
|-------|----------|----------|
| PM2 ecosystem | `INFRA_RUNBOOK.md` § 2 | Process definitions, commands, lifecycle |
| Cloudflare tunnel | `INFRA_RUNBOOK.md` § 3 | Ingress rules, DNS routes, troubleshooting |
| Network topology | `SERVICE_TOPOLOGY.md` § 1-2 | Ports, routing, communication matrix |

### 7.1 Configuration File Locations

| Config | Path |
|--------|------|
| PM2 ecosystem | `/Users/ubl-ops/Integration/ecosystem.config.cjs` |
| Cloudflare tunnel | `~/.cloudflared/config.yml` |
| Supabase migrations | `/Users/ubl-ops/Integration/logic.logline.world/supabase/migrations/` |

### 7.3 Environment Variables

**Shared (all services):**
```bash
SUPABASE_URL=https://aypxnwofjtdnmtxastti.supabase.co
SUPABASE_ANON_KEY=<anon-key>
SUPABASE_SERVICE_KEY=<service-role-key>  # Server-only, never in client
```

**llm-gateway specific:**
```bash
LLM_API_KEY=<gateway-master-key>
OPENAI_API_KEY=<optional>
ANTHROPIC_API_KEY=<optional>
GEMINI_API_KEY=<optional>
```

**code247 specific:**
```bash
LINEAR_API_KEY=<linear-key>
LINEAR_TEAM_ID=<team-id>
LLM_GATEWAY_URL=http://localhost:7700
LLM_GATEWAY_KEY=<service-api-key>
```

---

## 8) Migration from Current State

### 8.1 Current State Summary

| Service | Auth | LLM | Storage | Fuel |
|---------|------|-----|---------|------|
| llm-gateway | API key | N/A | SQLite | SQLite |
| code247 | None | Direct Anthropic/Ollama | SQLite | None |
| obs-api | Supabase JWT | Via gateway proxy | Supabase | N/A |
| CLI | Supabase JWT | N/A | Supabase | Supabase |

### 8.2 Target State Summary

| Service | Auth | LLM | Storage | Fuel |
|---------|------|-----|---------|------|
| llm-gateway | Supabase JWT + API key | N/A | Supabase | Supabase |
| code247 | Supabase JWT | Via llm-gateway | Supabase | Supabase |
| obs-api | Supabase JWT | Via gateway proxy | Supabase | N/A |
| CLI | Supabase JWT | N/A | Supabase | Supabase |

### 8.3 Migration Phases

See `INTEGRATION_PHASES.md` for detailed execution plan.

---

## 9) Invariants (Non-Negotiable)

### 9.1 Architecture Invariants

- `INV-001`: Business logic MUST reside in Rust crates, not UI handlers.
- `INV-002`: CLI MUST be first implementation surface for new capabilities.
- `INV-003`: All services MUST resolve identity scope before protected operations.
- `INV-004`: Cross-service calls MUST use documented API contracts.
- `INV-005`: Supabase is the sole persistence backend (no other databases).

### 9.2 Auth Invariants

- `INV-101`: All external-facing endpoints MUST validate auth.
- `INV-102`: Service-to-service auth MUST use Supabase JWT or registered API key.
- `INV-103`: Tokens MUST NOT be logged or persisted in plaintext.
- `INV-104`: JWT validation MUST check `exp`, `iss`, and `aud` claims.

### 9.3 Billing Invariants

- `INV-201`: Every billable action MUST emit a `fuel_event`.
- `INV-202`: `fuel_events` table MUST be append-only.
- `INV-203`: `idempotency_key` MUST prevent duplicate charges.
- `INV-204`: Pricing logic MUST NOT reside in apps.

### 9.4 Operational Invariants

- `INV-301`: All services MUST run under PM2 supervision.
- `INV-302`: All public DNS MUST route through Cloudflare Tunnel.
- `INV-303`: Secrets MUST NOT exist in git or env files on disk.
- `INV-304`: Production deployments MUST use `--release` builds.

---

## 10) Testable Acceptance Criteria

### Phase 1: Shared Crates
- [ ] `logline-supabase` crate compiles
- [ ] `logline-supabase` can emit fuel event to Supabase
- [ ] `logline-auth` validates Supabase JWT correctly

### Phase 2: llm-gateway Integration
- [ ] Gateway validates Supabase JWT
- [ ] Gateway emits fuel to Supabase on each request
- [ ] Gateway logs requests to `llm_requests` table
- [ ] Existing API key auth continues to work

### Phase 3: code247 Integration
- [ ] code247 validates Supabase JWT
- [ ] code247 persists jobs to Supabase
- [ ] code247 routes LLM calls through llm-gateway
- [ ] code247 broadcasts status via Supabase Realtime

### Phase 4: Unified Operations
- [ ] Single `ecosystem.config.cjs` manages all services
- [ ] Cloudflare tunnel routes all services correctly
- [ ] Health checks pass for all services
- [ ] Fuel dashboard shows unified metrics

---

## 11) References

- `ARCHITECTURE.md` — UI architecture details
- `LOGLINE_ECOSYSTEM_NORMATIVE_BASE.md` — Non-negotiable rules
- `ECOSYSTEM_PHASE0_APPROVED_V1.md` — Phase 0 decisions
- `SUPABASE_FOUNDATION.md` — Supabase setup
- `API_CONTRACTS.md` — API surface documentation
- `RBAC_MODEL.md` — Permission model
- `SERVICE_TOPOLOGY.md` — Network topology details
- `INTEGRATION_PHASES.md` — Execution timeline
- `INFRA_RUNBOOK.md` — PM2/Cloudflare operations

---

## Changelog

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-03-02 | UBL Founder | Initial blueprint |
