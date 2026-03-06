# edge-control.logline.world

Rust control-plane service for LogLine ecosystem.

Responsibilities:
- validate auth (JWT/internal token)
- enforce middleware gates (`request_id`, rate limit, idempotency)
- load and apply deterministic policy set (`policy-set.v1.1`)
- emit causal Fuel events (`*.opinion_emitted` -> `*.gate_decided`) with `trace_id` and `parent_event_id`
- expose typed endpoints:
  - `POST /v1/intention/draft`
  - `POST /v1/pr/risk`
  - `POST /v1/fuel/diff/route`
  - `POST /v1/orchestrate/intention-confirmed` (YES/HUMAN #1)
  - `POST /v1/orchestrate/github-event` (GitHub -> Linear sync via Code247)
  - `POST /v1/orchestrate/rollback` (YES/HUMAN #2)
- return JSON contracts aligned with `contracts/schemas` and `contracts/openapi`

## Run

```bash
cd edge-control.logline.world
cp .env.example .env
cargo run
```

Health check:

```bash
curl http://localhost:8080/health
```

Policy notes:
- Default policy path: `../policy/policy-set.v1.1.json`
- Override with `EDGE_CONTROL_POLICY_SET_PATH`
- Each protected endpoint emits a gate decision in:
  - response headers: `x-gate-decision`, `x-gate-policy`
  - structured logs (`gate decision emitted`)

Fuel notes:
- Requires `SUPABASE_URL` and `SUPABASE_SERVICE_ROLE_KEY`
- Fallback identity envs (for service tokens without full claims):
  - `EDGE_CONTROL_DEFAULT_TENANT_ID`
  - `EDGE_CONTROL_DEFAULT_APP_ID`
  - `EDGE_CONTROL_DEFAULT_USER_ID`
- Optional observability mirroring to `obs-api`:
  - `OBS_API_BASE_URL` (e.g. `https://obs-api.logline.world`)
  - `OBS_API_TOKEN` (JWT/service token with `obs:ingest` scope)

Orchestration notes:
- Requires `CODE247_BASE_URL` and `CODE247_INTENTIONS_TOKEN`
- Checkpoint enforcement:
  - `/v1/orchestrate/intention-confirmed` only accepts `YES_HUMAN_1`
  - `/v1/orchestrate/rollback` only accepts `YES_HUMAN_2`
