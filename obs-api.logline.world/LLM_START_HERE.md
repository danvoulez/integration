# LLM START HERE: OBS-API

## Authority

- Read model and observability hub only
- Never source of business truth
- Never decision engine

## Entry Files

- `app/api/`: HTTP routes
- `lib/obs/`: read-model queries and transformations
- `lib/auth/`: auth helpers
- `openapi.yaml`: public contract

## HTTP Surface

- `POST /api/v1/events/ingest`
- `GET /api/v1/fuel/dashboard`
- `GET /api/v1/fuel/alerts`
- `GET /api/v1/fuel/calibration`
- `GET /api/v1/fuel/reconciliation`
- `GET /api/v1/fuel/ops`
- `GET /api/v1/code247/stage-telemetry`
- `GET /api/v1/code247/run-timeline`
- `GET /api/health`

## Behavioral Rules

- Read from canonical backend state only
- No mock data in production code
- No hidden state machines here
- Protected endpoints require auth
- Event ingest requires `request_id`

## UI Rules

- UI comes after backend
- Do not invent fake readiness with placeholder data
- Prefer backend route completion before UI wiring

## Secrets

- Use `doppler run --project logline-ecosystem --config <env> -- npm run dev`
- Relevant secrets/config:
  - `SUPABASE_*`
  - `DATABASE_URL`/equivalent DB env
  - `SUPABASE_JWT_SECRET`

## Required Checks

```bash
cd obs-api.logline.world && npm run lint
cd /Users/ubl-ops/Integration && doppler run --project logline-ecosystem --config dev -- ./scripts/smoke-obs-api-auth.sh
```

## Do Not Do

- Do not add business decisions
- Do not accept unauthenticated write paths
- Do not ship dashboards backed by synthetic data

## Next Docs

- `README.md`
- `../TASKLIST-GERAL.md`
