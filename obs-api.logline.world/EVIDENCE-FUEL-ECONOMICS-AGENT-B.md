# Agent B Evidence - Fuel Economics and Obs Telemetry

Date: 2026-03-06
Branch: `codex/agent-b-llm-fuel-hardening`

## Scope
- `logic.logline.world/supabase/migrations/20260306000019_fuel_policy_alerts_ops.sql`
- `obs-api.logline.world/lib/obs/fuel.ts`
- `obs-api.logline.world/lib/obs/events.ts`
- `obs-api.logline.world/app/api/v1/fuel/{dashboard,reconciliation,alerts,calibration,ops}/route.ts`
- `obs-api.logline.world/openapi.yaml`

## Delivered
- Policy assignment by `tenant_id/app_id` with effective window in `fuel_policy_assignments`
- `fuel_points_v1` segmented by policy and enriched with provider/model/mode/fallback/timeout/token fields
- Drift, baseline and realtime views segmented by `policy_version`
- Daily calibration view: `fuel_policy_calibration_daily_v1`
- Daily policy/precision cuts: `fuel_policy_precision_daily_v1`
- Mode/provider effective-cost view: `fuel_llm_mode_provider_metrics_v1`
- Fuel alert candidate view: `fuel_alert_candidates_v1`
- Daily ops evidence table + RPC: `fuel_ops_job_runs`, `app.materialize_fuel_ops_jobs(...)`
- Obs endpoints:
  - `GET /api/v1/fuel/dashboard`
  - `GET /api/v1/fuel/reconciliation`
  - `GET /api/v1/fuel/alerts`
  - `GET /api/v1/fuel/calibration`
  - `GET /api/v1/fuel/ops`
  - `POST /api/v1/fuel/ops`

## Validation run
### llm-gateway
Command:
```bash
cargo test
```
Result:
```text
16 passed; 0 failed
```
Includes existing fallback/timeout/load tests for Agent B hardening.

### obs-api targeted lint
Command:
```bash
npx eslint lib/obs/fuel.ts lib/obs/events.ts app/api/v1/fuel/dashboard/route.ts app/api/v1/fuel/reconciliation/route.ts app/api/v1/fuel/alerts/route.ts app/api/v1/fuel/calibration/route.ts app/api/v1/fuel/ops/route.ts
```
Result:
```text
exit code 0
```

### obs-api full lint
Command:
```bash
npm run lint
```
Result:
```text
failed due to pre-existing unrelated errors in components/AppShell.tsx and lib/theme.tsx
```
Those failures are outside the Fuel/telemetry files changed here.

## Post-migration smoke queries
Apply migrations, then run:
```sql
select * from fuel_policy_assignments limit 5;
select * from fuel_alert_candidates_v1 order by detected_at desc limit 20;
select * from fuel_policy_calibration_daily_v1 order by day desc limit 20;
select * from fuel_llm_mode_provider_metrics_v1 order by day desc, usd_effective_total desc limit 20;
select * from app.materialize_fuel_ops_jobs('baseline_and_alerts', now());
```

## Expected backend checks
```bash
curl -H "Authorization: Bearer <jwt>" "http://localhost:3000/api/v1/fuel/dashboard?tenant_id=<tenant>&app_id=<app>&policy_version=<policy>"
curl -H "Authorization: Bearer <jwt>" "http://localhost:3000/api/v1/fuel/reconciliation?days=14&policy_version=<policy>&precision_level=L2"
curl -H "Authorization: Bearer <jwt>" "http://localhost:3000/api/v1/fuel/alerts?tenant_id=<tenant>&app_id=<app>"
curl -H "Authorization: Bearer <jwt>" "http://localhost:3000/api/v1/fuel/calibration?days=14&tenant_id=<tenant>&app_id=<app>"
curl -X POST -H "Authorization: Bearer <jwt>" -H "Content-Type: application/json" \
  -d '{"job_name":"baseline_and_alerts"}' \
  "http://localhost:3000/api/v1/fuel/ops"
```

## Acceptance mapping
- `G-024`: Fuel alerts persisted in `obs_alerts` and visible through `GET /api/v1/alerts/open` and `GET /api/v1/fuel/alerts`
- `G-024-C`: recurring baseline/alerts evidence materialized into `fuel_ops_job_runs`
- `G-025`: calibration view exposes daily distributions, p95 and k sensitivity deltas
- `G-026`: policy version resolution is segmented by tenant/app with effective dates
- `G-027`: dashboard and reconciliation accept `policy_version` and `precision_level` filters and expose cuts
- `LLM-011`: `obs-api` publishes by `mode/provider` request counts, fallback, timeout, settled ratio and effective cost per 1k tokens
