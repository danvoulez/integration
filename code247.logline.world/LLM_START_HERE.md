# LLM START HERE: CODE247

## Authority

- Owns coding pipeline state, execution evidence, Linear sync, PR/merge/deploy orchestration.
- `obs-api` is not authority for Code247 state.

## Entry Files

- `src/main.rs`: server/runtime wiring
- `src/api_rs.rs`: auth + HTTP surface
- `src/pipeline_rs.rs`: pipeline stages
- `src/persistence_rs.rs`: SQLite state + timeline + leases
- `src/transition_guard_rs.rs`: state transition rules
- `src/supabase_sync_rs.rs`: Supabase mirror + Realtime

## HTTP Surface

- `POST /intentions`
- `POST /intentions/sync`
- `GET /intentions/{workspace}/{project}`
- `GET /jobs`
- `GET /jobs/{job_id}/timeline`
- `GET /health`

## Auth Rules

- Preferred auth: Supabase HS256 JWT
- Legacy token: fallback only, controlled by `CODE247_AUTH_ALLOW_LEGACY_TOKEN=false` by default
- Required scopes:
  - `code247:intentions:write`
  - `code247:intentions:sync`
  - `code247:intentions:read`
  - `code247:jobs:read`
  - `code247:jobs:write`
  - `code247:admin`
- Project grants come from:
  - `code247_projects`
  - `projects`
  - or scopes `code247:project:<workspace>/<project>`

## State Invariants

- `Done` requires evidence
- `In Progress -> Done` is blocked
- `Ready for Release -> Done` requires deploy evidence
- `POST /intentions/sync` cannot move to `Done` without evidence
- Risk scoring is deterministic
- Pipeline is fail-closed

## Lease Invariants

- Stages `PLANNING/CODING/REVIEWING/VALIDATING/COMMITTING` use lease + heartbeat
- Expired lease becomes normal fail-closed state transition
- Claim of `PENDING` must be atomic
- Sweeper is lateral/idempotent, not authority

## Secrets

- Use `doppler run --project logline-ecosystem --config <env> -- cargo run`
- Do not treat `.env` as the normal runtime path
- Relevant secrets/config:
  - `SUPABASE_JWT_SECRET`
  - `SUPABASE_SERVICE_ROLE_KEY`
  - `LINEAR_*`
  - `GITHUB_TOKEN`
  - `OBS_API_TOKEN`

## Required Checks

```bash
cargo test --manifest-path code247.logline.world/Cargo.toml
./scripts/verify-code247-state-governance.sh
DB_PATH=/abs/path/to/db ./scripts/smoke-code247-stage-lease.sh
```

## Do Not Do

- Do not bypass transition guard
- Do not bypass lease rules
- Do not move Linear state directly without evidence
- Do not add special-case paths that skip audit/timeline/event emission

## Next Docs

- `README.md`
- `../TASKLIST-GERAL.md`
