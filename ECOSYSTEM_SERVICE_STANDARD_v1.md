# Ecosystem Service Standard v1

Status: ACTIVE  
Owner: LogLine Foundation  
Updated: 2026-03-05

## 1) Purpose

Define one operational standard for all ecosystem surfaces (CLI + APIs + observability) so UI can be built over a stable capability menu.

This document answers three questions:
- Is there an endpoint/command for each critical capability?
- Are services standardized enough to compose in UI?
- Is `obs-api` centralizing the right operational telemetry?

## 2) Canonical Capability Surfaces

### 2.1 CLI surface (operator-first)

- Every new capability MUST exist in CLI first (`logic.logline.world`) before API/UI exposure.
- CLI commands MUST be documented and runnable via deterministic examples.
- CLI output for automatable flows SHOULD be machine-readable (`json` mode where possible).

### 2.2 API surface (service-first)

Every externally consumed endpoint MUST have:
- Stable route and auth requirement.
- Input/output contract (schema URI).
- `request_id` for replay and audit.
- OpenAPI entry (public/internal depending on scope).

### 2.3 Event surface (observability-first)

Every critical operation MUST emit ingestible events to `obs-api` using:
- `event_id`, `event_type`, `occurred_at`, `source`, `request_id`
- `trace_id`, `parent_event_id` when causal linkage exists
- `intention_id/run_id/issue_id/pr_id/deploy_id` when available
- Rich `payload` for diagnosis

## 3) Mandatory API Standard

### 3.1 Success envelope

All non-streaming JSON endpoints MUST return:
- `request_id` (string, non-empty)
- `output_schema` (absolute schema URI)
- payload fields specific to endpoint contract

Header alignment:
- `x-request-id` MUST match `request_id`

### 3.2 Error envelope

Errors MUST use:
- `https://logline.world/schemas/error-envelope.v1.schema.json`
- deterministic `type` and actionable `message`

### 3.3 Auth and scope

- Service-to-service: `Authorization: Bearer <jwt_or_service_token>`
- Preferred: Supabase JWT with claims/scope; fallback internal token only for isolated internal traffic.
- Protected operations MUST resolve effective identity scope (`tenant_id`, `app_id`, `user_id`) before execution.

### 3.4 Contract governance

- Every service with external API MUST publish OpenAPI.
- `contracts_validate` MUST fail CI if contract artifacts are missing or invalid.
- Breaking contract changes MUST be versioned (`v1` -> `v2`) and migration-noted.

## 4) `obs-api` Centralization Rule

`obs-api` is the operational observability hub, not business authority.

`obs-api` MUST centralize:
- cross-service event ingest (`/api/v1/events/ingest`)
- timeline by intention (`/api/v1/timeline/:intentionId`)
- trace tree by causal id (`/api/v1/traces/:traceId`)
- run projection (`/api/v1/runs/:runId`)
- operator summary/alerts (`/api/v1/dashboards/summary`, `/api/v1/alerts/*`)

`obs-api` MUST NOT centralize:
- domain decision logic already owned by Rust control/runtime services
- hidden app-specific state machines outside canonical contracts

## 5) Definition of Ready for UI Capability

A capability is UI-ready only when all are true:

1. CLI command exists and is documented.
2. API endpoint exists with stable auth + OpenAPI + schemas.
3. Success/error envelopes are standard-compliant.
4. Events are emitted and visible in `obs-api` timeline/trace.
5. CI validates contracts and basic contract tests.

## 6) Convergence Checklist (All Services)

- [ ] STD-001: All JSON APIs return `request_id` + `output_schema`.
- [ ] STD-002: All JSON API errors use `error-envelope.v1`.
- [ ] STD-003: All externally consumed endpoints are present in OpenAPI.
- [ ] STD-004: `contracts_validate` enforces every service OpenAPI.
- [ ] STD-005: All services emit causal telemetry to `obs-api` ingest.
- [ ] STD-006: All protected endpoints enforce bearer auth + scope resolution.
- [ ] STD-007: Each capability has CLI or API cookbook entry (input/output/auth/examples).

## 7) Service-by-Service Checklist (Current Snapshot)

### 7.1 logic.logline.world (CLI authority)

- [x] LGS-001: Core operator commands documented in README.
- [x] LGS-002: CLI mirrors runtime checkpoints to `obs-api`.
- [x] LGS-003: Publish canonical CLI command catalog JSON (for cookbook generation) via `logline catalog export` + `contracts/generated/capability-catalog.v1.json`.

### 7.2 edge-control.logline.world (control plane)

- [x] ECS-001: Protected `/v1/*` endpoints with request-id/rate-limit/idempotency middleware.
- [x] ECS-002: Typed responses include `request_id` + `output_schema`.
- [x] ECS-003: OpenAPI contract exists (`contracts/openapi/edge-control.v1.openapi.yaml`).
- [x] ECS-004: Emits causal fuel + mirrors to `obs-api` ingest.

### 7.3 llm-gateway.logline.world

- [x] LLM-STD-001: OpenAPI contract exists (`llm-gateway.logline.world/openapi.yaml`).
- [x] LLM-STD-002: Main chat response includes `request_id` + `output_schema`.
- [x] LLM-STD-003: Emits telemetry to `obs-api` ingest.
- [ ] LLM-STD-004: Ensure non-chat JSON endpoints also follow success/error envelope standard.

### 7.4 code247.logline.world

- [x] C247-STD-001: Critical endpoints exist (`/intentions`, `/intentions/sync`, webhook/OAuth).
- [x] C247-STD-002: Emits operational events to `obs-api` ingest.
- [x] C247-STD-003: Add canonical `output_schema` on all JSON success responses.
- [x] C247-STD-004: Add canonical `error-envelope.v1` on all JSON errors.
- [x] C247-STD-005: Publish and validate OpenAPI spec in CI.

### 7.5 obs-api.logline.world

- [x] OBS-STD-001: Ingest/timeline/trace/run/summary/alerts endpoints are restored.
- [x] OBS-STD-002: Scope-gated ingest/read/ack auth is active.
- [x] OBS-STD-003: Standardize endpoint responses with `request_id` + `output_schema`.
- [x] OBS-STD-004: Standardize errors to `error-envelope.v1`.
- [x] OBS-STD-005: Publish and validate OpenAPI spec in CI.

## 8) Execution Order (Recommended)

1. Enforce response envelope in `code247` and `obs-api`.
2. Publish OpenAPI for `code247` and `obs-api`.
3. Extend `contracts_validate` to include these OpenAPI files.
4. Generate ecosystem cookbook from CLI catalog + OpenAPI.
5. Wire UI only to cookbook-listed capabilities.
