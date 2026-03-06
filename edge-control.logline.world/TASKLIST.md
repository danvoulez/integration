# edge-control Tasklist

## H0
- [x] EDGE-001: Bootstrap Rust service with auth + request_id + rate-limit + idempotency middleware.
- [x] EDGE-002: Implement `/v1/intention/draft`, `/v1/pr/risk`, `/v1/fuel/diff/route` endpoints.
- [x] EDGE-003: Load and apply `policy/policy-set.v1.1.json` to emit `GateDecision.v1`.
- [x] EDGE-004: Emit causal Fuel events to Supabase (`event_type`, `trace_id`, `parent_event_id`).
- [x] EDGE-005: Integrate Linear + Code247 orchestration loop.

## H1
- [ ] EDGE-006: Replace HS256 shortcut with JWKS validation for Supabase JWT.
- [ ] EDGE-007: Persist idempotency keys in durable store (Redis/Postgres) for multi-instance safety.
- [ ] EDGE-008: Add OpenAPI contract tests against handlers.
- [x] EDGE-009: Mirror Fuel causal events to `obs-api /api/v1/events/ingest`.
