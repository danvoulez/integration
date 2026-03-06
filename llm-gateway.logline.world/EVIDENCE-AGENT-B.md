# Agent B Evidence (2026-03-06)

## 1) Latency local before/after (p95/p99)

Command:

```bash
cargo test agent_b_ -- --nocapture
```

Output excerpt:

```text
agent_b_latency_before_after local p95/p99: before=89/89 after=45/45
test tests::agent_b_load_short_burst_improves_p95_p99_with_local_token_caps ... ok
```

Interpretation: short burst load test showed p95 and p99 improvement with new local max-token adaptive caps.

## 2) Fallback + timeout degradation tests

Same command (`cargo test agent_b_ -- --nocapture`) validated:

- `agent_b_fallback_local_timeout_to_openai_succeeds ... ok`
- `agent_b_timeout_when_local_queue_wait_exceeded ... ok`

This covers controlled fallback and queue-timeout behavior.

## 3) Regression sweep

Command:

```bash
cargo test
```

Result:

```text
test result: ok. 16 passed; 0 failed; 0 ignored
```

## 4) Economic telemetry and settlement deliverables in this cycle

- Supabase fuel primary path with SQLite fallback counters.
- Cloud settlement endpoint with provider API pull + retry/backoff.
- Local energy valuation upsert with explicit method/confidence.
- Normalized `llm_requests` contract fields (`request_id`, `plan_id`, `ci_target`, `fallback_behavior`).
