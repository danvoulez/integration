# LLM Gateway

LLM Gateway is the lightweight app that exposes the canonical OpenAPI surface and runtime hooks for the broader logline.world ecosystem. It plays as an “app” layer (alongside Code247, logic, obs-api) and speaks the same canonical manifest + Linear inputs defined in `canon/workspace-ast/0.1`.

## Quickstart

```bash
cd llm-gateway.logline.world
cargo build
cargo run
```

### Validation

- Manifest (mandatory): `.code247/workspace.manifest.json` (schema: `schemas/workspace.manifest.schema.json`).
- Validation command: `cargo check` (per manifest gates).
- Optional: `cargo test` for deeper coverage.

## Architecture summary

- **Purpose**: provide a canonical gateway for model routing and schema exposure. It hosts `openapi.yaml`, `schemas/`, and can be referenced by other apps.
- **Stack**: Rust (Tokio + Axum + OpenAPI), outputs `ecosystem.config.cjs` for deployment, publishes telemetry via `gateway.log`.
- **Contracts**: OpenAPI spec (`openapi.yaml`) plus the shared workspace manifest and Linear schema under `schemas/`.
- **Observability integration**: optional mirror to `obs-api` via
  - `OBS_API_BASE_URL`
  - `OBS_API_TOKEN` (`obs:ingest` scope)

## Development

- Build: `cargo build` (gate0 commands: `cargo check`, `cargo test`).
- Tests: `cargo test`; `cargo fmt --check`, `cargo clippy --all-targets` as needed.
- Linear input: `.code247/workspace.manifest.json` ensures `inputs.linear` configuration matches the canonical plugin.

## Local latency controls

- `LLM_LOCAL_MAX_QUEUE_WAIT_MS` bounds how long a request waits for a local model slot before fallback.
- Monitor `llm_gateway_local_queue_timeouts_total` in `/metrics`; sustained growth means local concurrency is saturated.
- Adaptive local tuning (p95/p99-driven) is enabled by default and adjusts:
  - local queue wait budget (`baseline/degraded/emergency`),
  - local model caps (`num_ctx`, `num_batch`),
  - warmup cadence and timeout.
- Adaptive tuning env knobs:
  - `LLM_LOCAL_ADAPTIVE_TUNING_ENABLED`
  - `LLM_LOCAL_ADAPTIVE_MIN_SAMPLES`
  - `LLM_LOCAL_ADAPTIVE_P95_DEGRADED_MS`
  - `LLM_LOCAL_ADAPTIVE_P99_EMERGENCY_MS`
  - `LLM_LOCAL_ADAPTIVE_DEGRADED_QUEUE_WAIT_MS`
  - `LLM_LOCAL_ADAPTIVE_EMERGENCY_QUEUE_WAIT_MS`
  - `LLM_LOCAL_ADAPTIVE_DEGRADED_NUM_CTX_CAP`
  - `LLM_LOCAL_ADAPTIVE_DEGRADED_NUM_BATCH_CAP`
  - `LLM_LOCAL_ADAPTIVE_EMERGENCY_NUM_CTX_CAP`
  - `LLM_LOCAL_ADAPTIVE_EMERGENCY_NUM_BATCH_CAP`
- Adaptive profile counters are exposed in `/metrics` as `llm_gateway_local_adaptive_profile_total{profile=...}`.

## Stable mode contract

- Canonical modes for callers: `genius`, `fast`, `code`.
- Legacy aliases remain accepted and normalized server-side:
  - `premium -> genius`
  - `local -> code`
  - `auto -> code`
- Contract endpoint: `GET /v1/modes`.
- Code247 contract endpoint: `GET /v1/contracts/code247` (includes `ci_target` + `fallback_behavior` fields).

## Dashboard metrics API

- Prometheus endpoint: `GET /metrics`.
- JSON summary endpoint for dashboards/obs-api: `GET /v1/metrics/summary`.

## Internal security controls

- `LLM_RATE_LIMIT_PER_MINUTE` applies fixed-window per-client/app rate limiting on generation endpoints.
- `SUPABASE_REQUIRED_SERVICE_SCOPE` (optional) enforces a required scope on Supabase service JWTs (ex: `llm:invoke`).
- Legacy API key compat controls:
  - `LLM_LEGACY_API_KEY_MODE` = `compat` | `disabled` | `legacy_only`
  - `LLM_LEGACY_API_KEY_SUNSET_AT` (RFC3339 cutoff for compat mode)

## Fuel reconciliation and settlement

- Supabase is the primary economic ledger path when `SUPABASE_FUEL_PRIMARY_ENABLED=true`.
- Cloud settlement endpoint: `POST /v1/admin/fuel/reconcile/cloud`.
  - Reconciles OpenAI (`/v1/organization/usage/completions`, `/v1/organization/costs`) and Anthropic (`/v1/organizations/usage_report/messages`, `/v1/organizations/cost_report`).
  - Uses idempotent settlement run ids and retry/backoff knobs under `SUPABASE_SETTLEMENT_*`.
- Local energy valuation is written with confidence/method metadata for local provider calls.

## Release manifest

- Admin endpoint: `POST /v1/admin/release/manifest` writes `manifest.intentions.json`.
- Optional startup autopublish:
  - `LLM_RELEASE_AUTOPUBLISH=true`
  - `LLM_RELEASE_VERSION=<semver>`
  - `LLM_INTENTION_MANIFEST_PATH=<path>`

## Deployment/readiness

1. Ensure manifest passes schema validation (`cargo run --bin validate_manifest` if added).
2. Run gate0 commands: `cargo check`, `cargo test`.
3. Confirm `openapi.yaml` is up-to-date and referenced from manifest `contracts.openapi_paths`.
4. Ship via `ecosystem.config.cjs` (Cloudflare/hosting) and monitor `gateway.log`.

## Auth Smoke

Use `Doppler` as the canonical secret source:

```bash
cd /Users/ubl-ops/Integration
doppler run --project logline-ecosystem --config dev -- ./scripts/smoke-llm-gateway-auth.sh
```

The smoke validates:
- Supabase JWT service token accepted in the canonical path
- legacy API key accepted in `compat` mode
- legacy API key rejected when `LLM_LEGACY_API_KEY_MODE=disabled`

## Contacts and docs

- Manifest & canon: `.code247/workspace.manifest.json`, `/Users/ubl-ops/Integration/canon/workspace-ast/0.1`.
- Schema sources: `schemas/workspace.manifest.schema.json`, `schemas/workspace.manifest.strict.schema.json`, `schemas/inputs.linear.schema.json`.
- OpenAPI contract: `openapi.yaml`.
