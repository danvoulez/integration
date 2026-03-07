# LLM START HERE: EDGE-CONTROL

## Authority

- Owns deterministic policy gating at the edge
- Owns orchestration entrypoints into `Code247`
- Owns causal Fuel emission for edge opinions/decisions

## Entry Files

- `src/main.rs`: app wiring
- `src/auth.rs`: inbound auth
- `src/middleware.rs`: request id, auth, rate limit, idempotency
- `src/policy.rs`: deterministic policy evaluation
- `src/handlers.rs`: HTTP handlers
- `src/orchestration.rs`: `edge-control -> Code247`
- `src/fuel.rs`: fuel ledger emission
- `src/state_store.rs`: idempotency backend

## HTTP Surface

- `POST /v1/intention/draft`
- `POST /v1/pr/risk`
- `POST /v1/fuel/diff/route`
- `POST /v1/orchestrate/intention-confirmed`
- `POST /v1/orchestrate/github-event`
- `POST /v1/orchestrate/rollback`
- `GET /health`

## Auth Rules

- Inbound preferred auth: Supabase JWKS/JWT
- Inbound fallback: internal bearer only when configured
- Outbound to `Code247`: short-lived Supabase HS256 service JWT
- Legacy `CODE247_INTENTIONS_TOKEN`: fallback only

## Decision Rules

- LLM/opinion never executes directly
- `edge-control` decides via deterministic policy
- All protected routes emit `x-gate-decision` and `x-gate-policy`
- Human checkpoints:
  - `YES_HUMAN_1` only for intention confirmation
  - `YES_HUMAN_2` only for rollback approval

## Persistence/Infra Rules

- Idempotency backend: `auto|supabase|sqlite`
- Operational default: shared Supabase path
- SQLite is fallback for dev/test only
- Supabase is primary for Fuel events and shared idempotency

## Secrets

- Use `doppler run --project logline-ecosystem --config <env> -- cargo run`
- Do not use `.env` as standard runtime
- Relevant secrets/config:
  - `SUPABASE_URL`
  - `SUPABASE_SERVICE_ROLE_KEY`
  - `SUPABASE_JWT_SECRET`
  - `SUPABASE_JWKS_URL`
  - `OBS_API_TOKEN`

## Required Checks

```bash
cargo test --manifest-path edge-control.logline.world/Cargo.toml
```

## Do Not Do

- Do not bypass policy gates
- Do not auto-approve human checkpoints
- Do not emit gate/fuel events without causal linkage
- Do not revert shared idempotency to in-memory-only behavior

## Next Docs

- `README.md`
- `../TASKLIST-GERAL.md`
