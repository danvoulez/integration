# LLM Gateway Tasklist

**Updated:** 2026-03-05

## Integration work
- [x] P2-001: Combine API key flow with Supabase JWT validation (`logline-auth`).
- [x] P2-002: Emit fuel events to Supabase (`llm_requests`) and remove local SQLite dependency.
- [x] P2-003: Log all incoming requests to `llm_requests` table with normalized request_id + plan_id.
- [x] P2-004: Mirror request telemetry to `obs-api /api/v1/events/ingest` with `request_id/trace_id`.

## Maintenance
- [x] Document the gateway-to-code247 contract (include `ci_target` for `code247-ci/main`).
- [x] Add contract tests verifying `logline-auth` tokens, request envelope, and `request_id` tracing.
- [x] Publish intention manifest after each release so Code247 can create the matching Linear issue.

## Performance/observability hardening
- [x] LLM-PW-001: Add rolling latency quantiles (`p50/p95/p99`) to `/metrics` by `mode`.
- [x] LLM-PW-002: Expand latency quantiles in `/metrics` by `provider` and `model`.
- [x] LLM-PW-003: Expose request error counters by `provider` and `model` in `/metrics` and `/fuel`.
- [x] LLM-PW-004: Apply local-model latency optimization actions based on the new quantiles (queue, warmup, model params).
- [x] LLM-PW-004-A: Add bounded local queue wait (`LLM_LOCAL_MAX_QUEUE_WAIT_MS`) with fast fallback and timeout telemetry.

## Auth/rate-limit hardening
- [x] LLM-AUTH-001: Add per-client/app fixed-window rate limiting on generation endpoints.
- [x] LLM-AUTH-002: Add optional required scope validation for Supabase service JWTs (`SUPABASE_REQUIRED_SERVICE_SCOPE`).
