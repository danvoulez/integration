# LogLine Ecosystem — LLM Start Here

**Read this before doing anything.** This is the normative guide for AI agents working in this ecosystem.

## What is LogLine?

LogLine is a Rust-first ecosystem with 5 core services:

| Service | Purpose | Stack |
|---------|---------|-------|
| `logic.logline.world` | CLI + Core Crates (HQ) | Rust workspace |
| `llm-gateway.logline.world` | LLM Routing + Billing | Rust binary |
| `code247.logline.world` | Autonomous Coding Agent | Rust binary |
| `edge-control.logline.world` | Control Plane (policy, orchestration) | Rust binary |
| `obs-api.logline.world` | Observability Dashboard | Next.js |

## Architecture Principles

1. **Rust owns business logic** — CLI and services are authoritative
2. **Supabase owns state** — Postgres, Auth, Realtime, Storage
3. **Contract-first** — OpenAPI, JSON schemas, events registry
4. **Fuel is the common currency** — tracks usage, cost, and operational health

## Key Documents (in this folder)

| Document | Purpose |
|----------|---------|
| `ARCHITECTURE.md` | **Source of Truth** — architecture and integration contracts |
| `SERVICE_TOPOLOGY.md` | Network, ports, DNS, communication matrix |
| `ECOSYSTEM_SERVICE_STANDARD_v1.md` | API/CLI/event standards |
| `FUEL_SYSTEM_SPEC.md` | **Fuel specification** — 3-layer model, schemas, calculation, visualization |
| `code247-intentions.md` | CI/CD trigger contract for all projects |
| `TASKLIST-GERAL.md` | Current sprint backlog |

## Service-Specific Guides

Each service has its own `LLM_START_HERE.md`:
- `code247.logline.world/LLM_START_HERE.md`
- `llm-gateway.logline.world/LLM_START_HERE.md`
- `edge-control.logline.world/LLM_START_HERE.md`
- `obs-api.logline.world/LLM_START_HERE.md`
- `logic.logline.world/docs/LLM_START_HERE.md`

## What You MUST NOT Do

1. **Never bypass CLI governance** — all capabilities must exist in CLI first
2. **Never hardcode secrets** — use Doppler (`doppler run`)
3. **Never emit events without `request_id` and `trace_id`**
4. **Never mark Linear issues as `Done` without evidence**
5. **Never touch `contracts/*`, `policy/*`, `openapi/*` without validation

## What You SHOULD Do

1. Read the service-specific `LLM_START_HERE.md` before editing that service
2. Check `TASKLIST-GERAL.md` for current priorities
3. Run `./scripts/validate-contracts.sh` after touching contracts
4. Emit fuel events for billable operations
5. Follow the Transistor pattern: LLM emits opinion → Rust judge decides

## Quick Commands

```bash
# Validate all contracts
./scripts/validate-contracts.sh

# Run integration smoke test
./scripts/smoke.sh

# Check PM2 services
pm2 status

# View logs
pm2 logs <service> --lines 50
```
