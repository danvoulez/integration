# LLM START HERE: LLM-GATEWAY

## Authority

- Canonical LLM routing surface for the ecosystem
- Owns provider selection, rate limiting, request accounting, fuel emission

## Entry Files

- `src/main.rs`: server wiring
- `src/routing.rs`: mode/provider routing
- `src/providers.rs`: provider adapters
- `src/fuel.rs`: fuel/accounting emission
- `src/batch.rs`: batch paths
- `openapi.yaml`: canonical contract

## HTTP Surface

- `POST /v1/chat/completions`
- `GET /v1/modes`
- `GET /v1/contracts/code247`
- `GET /v1/metrics/summary`
- `GET /metrics`
- `POST /v1/admin/fuel/reconcile/cloud`

## Auth Rules

- Preferred auth: Supabase JWT with scope enforcement
- Legacy API key mode exists only for controlled sunset
- Never expand legacy mode casually

## Economic Rules

- Every billable request must write:
  - `fuel_events`
  - `llm_requests`
- Supabase is primary ledger
- Optional mirror to `obs-api` is secondary only

## Mode Contract

- Canonical modes:
  - `genius`
  - `fast`
  - `code`
- Aliases normalize server-side only

## Secrets

- Use `doppler run --project logline-ecosystem --config <env> -- cargo run`
- Relevant secrets/config:
  - provider API keys
  - `SUPABASE_*`
  - `OBS_API_TOKEN`

## Required Checks

```bash
cargo check --manifest-path llm-gateway.logline.world/Cargo.toml
cargo test --manifest-path llm-gateway.logline.world/Cargo.toml
```

## Do Not Do

- Do not skip fuel emission
- Do not log provider secrets
- Do not bypass rate limiting
- Do not make SQLite the primary ledger path

## Next Docs

- `README.md`
- `RUNBOOK.md`
- `../TASKLIST-GERAL.md`
