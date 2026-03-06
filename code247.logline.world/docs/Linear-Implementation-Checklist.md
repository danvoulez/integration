# Code247 x Linear Implementation Checklist

## Phase 1 - Auth and Security

- [ ] L247-001: Implement `/oauth/start` with `actor=app`, `state`, and scope control.
- [ ] L247-002: Implement `/oauth/callback` token exchange and secure persistence.
- [ ] L247-003: Implement proactive refresh-token worker.
- [ ] L247-004: Add secret rotation runbook for OAuth and webhook keys.

## Phase 2 - Webhook Intake

- [ ] L247-005: Implement `POST /webhooks/linear` raw-body signature validation.
- [ ] L247-006: Enforce timestamp skew window (`webhookTimestamp`).
- [ ] L247-007: Add delivery idempotency store keyed by `Linear-Delivery`.
- [ ] L247-008: Add enqueue-first ack pattern (`verify -> dedupe -> enqueue -> 200`).

## Phase 3 - Runtime Contract

- [ ] L247-009: Implement issue claim/lock flow (`code247:locked` + start comment).
- [ ] L247-010: Implement duplicate/stale lock handling with TTL.
- [ ] L247-011: Implement automation labels (`queue`, `running`, `needs-cloud-review`, `validated`).
- [ ] L247-012: Implement cloud re-evaluation contract through labels/comments.

## Phase 4 - Completion and Evidence

- [ ] L247-013: Implement evidence comment template (plan, PR, gates, how_to_test).
- [ ] L247-014: Enforce `CI gate before Done` transition policy.
- [ ] L247-015: Add final audit payload per run_id.

## Phase 5 - Operability

- [ ] L247-016: Add webhook ingest metrics and alerts.
- [ ] L247-017: Add DLQ + replay command for failed webhook jobs.
- [ ] L247-018: Add SLO dashboards for latency/failure/dedupe.
- [ ] L247-019: Add conformance tests for signature, skew, dedupe, done-gate.

## Definition of Done

- [ ] All Phase 1-5 tasks complete.
- [ ] Manifest validation passes strict + linear extension schemas.
- [ ] End-to-end test: issue -> run -> PR -> merge -> evidence -> done.
- [ ] Production runbook executed successfully by operations.
