# LLM Gateway — LLM Start Here

**Read the root `LLM_START_HERE.md` first.**

## What is LLM Gateway?

The LLM Gateway is the canonical routing layer for all LLM requests in the ecosystem. It handles provider selection, billing, rate limiting, and telemetry.

## Architecture

```
Client → Gateway → Provider (OpenAI/Anthropic/Ollama) → Response
           ↓
    Supabase (fuel_events, llm_requests)
           ↓
    obs-api (telemetry mirror)
```

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point |
| `src/routing.rs` | Model/provider routing |
| `src/providers.rs` | Provider implementations |
| `src/fuel.rs` | Fuel event emission |
| `src/batch.rs` | Batch processing |
| `openapi.yaml` | API contract |

## Modes Contract

| Mode | Purpose | Typical Provider |
|------|---------|------------------|
| `genius` | Best quality, higher cost | Claude/GPT-4 |
| `fast` | Low latency | GPT-3.5/local |
| `code` | Code-optimized | Codestral/local |

Legacy aliases (normalized server-side): `premium → genius`, `local → code`, `auto → code`

## Critical Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /v1/chat/completions` | Main generation endpoint |
| `GET /v1/modes` | List available modes |
| `GET /v1/contracts/code247` | Code247 contract |
| `GET /metrics` | Prometheus metrics |
| `GET /v1/metrics/summary` | JSON metrics for dashboards |
| `POST /v1/admin/fuel/reconcile/cloud` | Cloud settlement |

## Auth Model

1. **Supabase JWT** (preferred) — validates `scope` claims
2. **Legacy API key** (compat mode) — controlled sunset via `LLM_LEGACY_API_KEY_SUNSET_AT`

Rate limiting: `LLM_RATE_LIMIT_PER_MINUTE` per client/app

## Fuel Emission

Every billable request emits to:
1. `fuel_events` (Supabase) — append-only ledger
2. `llm_requests` (Supabase) — normalized request log
3. `obs-api` (optional mirror) — via `OBS_API_BASE_URL`

## What You MUST NOT Do

1. **Never skip fuel event emission** for billable requests
2. **Never expose raw provider API keys** in logs
3. **Never bypass rate limiting**
4. **Never return responses without `request_id`**

## What You SHOULD Do

1. Always include `request_id` in responses and `x-request-id` header
2. Emit metrics by mode/provider/model
3. Use adaptive tuning for local models
4. Persist to Supabase as primary (not SQLite)

## Quick Commands

```bash
# Run locally
cargo run

# Check/test
cargo check && cargo test

# Format and lint
cargo fmt --check && cargo clippy --all-targets
```

## Key Docs

- `README.md` — Full reference with env vars
- `BLUEPRINT.md` — Mission and objectives
- `RUNBOOK.md` — Operational procedures
- `TASKLIST.md` — Current backlog
