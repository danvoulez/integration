# Fuel App Implementation Checklist

**Date:** 2026-03-05  
**Goal:** move Fuel from raw tracking to reconciled control currency.

## 1) llm-gateway

- [x] Emit `fuel_events` for billable requests (`unit_type=llm_tokens`).
- [x] Include causal metadata (`event_type`, `trace_id`, `outcome`, IDs).
- [ ] Add `cached_input_tokens`, `ttft_ms`, `retry_count`, `fallback_used` with real values (not placeholders).
- [ ] Add valuation worker input hooks (provider/model/mode/source normalized).
- [ ] Add reconciliation job wiring (usage/cost APIs -> `fuel_valuations`).

## 2) edge-control

- [x] Emit semantic/causal events (`event_type`, `trace_id`, `parent_event_id`, `reason_codes`).
- [ ] Expand `unit_type` beyond `api_call` where applicable (`policy_decision`, `intention_draft`, etc).
- [ ] Emit explicit reliability metadata (`retry_count`, `fallback_used`) when orchestration retries happen.

## 3) code247

- [ ] Emit `fuel_events` directly for stage execution (`planning`, `coding`, `review`, `validate`, `merge`).
- [ ] Map stage outputs to stable `unit_type` (`code_event`, `compute_seconds`, `llm_tokens` when available).
- [ ] Attach causal links (`job_id`, `issue_id`, `pr_id`, `trace_id`) in metadata.
- [ ] Guarantee idempotency for stage retries.

## 4) obs-api

- [ ] Add dashboard panel: `estimated vs settled` drift.
- [ ] Add dashboard panel: precision coverage (`L0/L1/L2/L3`).
- [ ] Add dashboard panel: fuel points decomposition (`base_cost`, `latency`, `errors`, `energy`).
- [ ] Add alert views (`fuel_points_rate`, `error_rate_burn`, `cost_spike`).

## 5) Supabase / Logic

- [x] Add Layer B schema (`fuel_valuations`) and Layer C derived view (`fuel_points_v1`).
- [x] Add control policy table (`fuel_control_policy_versions`) and default policy.
- [x] Add versioned price cards (`fuel_price_cards`).
- [ ] Add valuation/reconciliation worker job (L0 -> L1 upgrades).
- [ ] Add scheduled snapshots/diffs for recurring analysis.

## 6) Execution order

1. Deploy migration `20260305000011_fuel_valuation_points.sql`.
2. Start generating Layer A traffic with rich metadata.
3. Backfill `fuel_valuations` in L0 using price cards.
4. Turn on cloud reconciliation for L1.
5. Turn on dashboard + alerts over `fuel_points_v1`.
