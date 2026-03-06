# OBS API — LLM Start Here

**Read the root `LLM_START_HERE.md` first.**

## What is OBS API?

OBS API is the observability hub for the LogLine ecosystem. It centralizes event ingestion, timelines, traces, and operator dashboards. It is **not** a business authority — it observes and displays, never decides.

## Architecture

```
Services (code247, llm-gateway, edge-control) → /api/v1/events/ingest → Supabase
                                                         ↓
                                              Dashboards / Alerts / Timelines
```

## Key Directories

| Directory | Purpose |
|-----------|---------|
| `app/` | Next.js app routes |
| `app/api/` | API endpoints |
| `components/` | React components |
| `lib/` | Utilities and auth |
| `db/` | Database schema (Drizzle) |

## Critical Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/v1/events/ingest` | Cross-service event intake |
| `GET /api/v1/timeline/:intentionId` | Timeline by intention |
| `GET /api/v1/traces/:traceId` | Trace tree by causal ID |
| `GET /api/v1/runs/:runId` | Run projection |
| `GET /api/v1/dashboards/summary` | Operator summary |
| `GET /api/v1/fuel/dashboard` | Fuel metrics |
| `GET /api/health` | Health check |

## Event Ingestion Contract

Every event must include:
- `event_id` — unique identifier
- `event_type` — semantic type
- `occurred_at` — timestamp
- `source` — emitting service
- `request_id` — for replay/audit

When causal linkage exists:
- `trace_id`
- `parent_event_id`

When applicable:
- `intention_id`, `run_id`, `issue_id`, `pr_id`, `deploy_id`

## OBS API Rules

**MUST centralize:**
- Cross-service event ingest
- Timeline by intention
- Trace tree by causal ID
- Run projection
- Operator summary/alerts

**MUST NOT centralize:**
- Domain decision logic (owned by Rust services)
- Hidden app-specific state machines

## What You MUST NOT Do

1. **Never implement business logic here** — obs-api observes, never decides
2. **Never accept events without `request_id`**
3. **Never skip auth for protected endpoints**
4. **Never use mock data in production code**

## What You SHOULD Do

1. Display data from canonical backend endpoints
2. Implement proper error envelopes
3. Use Supabase JWT for auth
4. Keep dashboards real-time via Supabase Realtime

## Quick Commands

```bash
# Run dev server
npm run dev

# Type check
npm run typecheck

# Lint
npm run lint
```

## Key Docs

- `README.md` — Quick start, endpoints, project structure
- `../ECOSYSTEM_SERVICE_STANDARD_v1.md` — API standards
- `../FUEL_SYSTEM_SPEC.md` — Fuel system specification
