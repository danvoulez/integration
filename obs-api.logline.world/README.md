# OBS API

Observability hub for the LogLine ecosystem.

## Purpose

OBS API centralizes event ingestion, timelines, traces, and operator dashboards for all ecosystem services. It observes and displays but never makes business decisions.

## Quick Start

```bash
cd obs-api.logline.world
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Stack

- **Framework:** Next.js 14 (App Router)
- **Database:** Supabase Postgres (via Drizzle ORM)
- **Auth:** Supabase JWT
- **Realtime:** Supabase Realtime channels

## Key Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/v1/events/ingest` | Cross-service event intake |
| `GET /api/v1/timeline/:intentionId` | Timeline by intention |
| `GET /api/v1/traces/:traceId` | Trace tree by causal ID |
| `GET /api/v1/runs/:runId` | Run projection |
| `GET /api/v1/code247/stage-telemetry` | Latência/custo por etapa do pipeline |
| `GET /api/v1/fuel/dashboard` | Fuel metrics |
| `GET /api/v1/dashboards/summary` | Operator summary |
| `GET /api/health` | Health check |

## Event Ingestion

All ecosystem services emit events to `/api/v1/events/ingest`:

```json
{
  "event_id": "uuid",
  "event_type": "code247.job.started",
  "occurred_at": "2026-03-06T12:00:00Z",
  "source": "code247",
  "request_id": "req-uuid",
  "trace_id": "trace-uuid",
  "payload": {}
}
```

## Environment Variables

```bash
# Supabase
SUPABASE_URL=https://...supabase.co
SUPABASE_ANON_KEY=...
SUPABASE_SERVICE_ROLE_KEY=...   # Server-only

# Database
DATABASE_URL=postgresql://...

# Auth
SUPABASE_JWT_SECRET=...
```

## Project Structure

```
obs-api.logline.world/
├── app/
│   ├── api/           # API routes
│   │   ├── health/
│   │   └── v1/
│   ├── apps/          # Dashboard pages
│   └── page.tsx       # Home
├── components/        # React components
├── db/               # Drizzle schema
└── lib/              # Utilities
```

## Development

```bash
npm run dev          # Start dev server
npm run build        # Production build
npm run typecheck    # Type checking
npm run lint         # Linting
```

## Authenticated Smoke

```bash
cd /Users/ubl-ops/Integration
doppler run -- ./scripts/smoke-obs-api-auth.sh
```

O smoke sobe o `obs-api` localmente, emite JWT HS256 com escopos `obs:read` e `obs:alerts:ack`, valida os endpoints Fuel principais e grava:

- `/Users/ubl-ops/Integration/artifacts/obs-api-auth-smoke-report.json`
- `/Users/ubl-ops/Integration/artifacts/obs-api-auth-smoke.log`

## Related Docs

- [LLM_START_HERE.md](LLM_START_HERE.md) — Guide for AI agents
- [../ECOSYSTEM_SERVICE_STANDARD_v1.md](../ECOSYSTEM_SERVICE_STANDARD_v1.md) — API standards
