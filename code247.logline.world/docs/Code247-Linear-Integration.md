# Code247 x Linear Integration Spec

## 1. Objective

Transform Linear into the control plane for Code247 while preserving Code247 as the 24/7 executor.

Linear responsibilities:
- Intake and prioritization.
- State transitions and human commands.
- Audit trail for run decisions and outcomes.

Code247 responsibilities:
- Planner -> Coder -> Reviewer -> Gates -> PR/Merge execution.
- Idempotent runtime with retry/fallback policies.
- Evidence publication back to Linear before marking work as done.

## 2. Integration Principles

- Auth model: OAuth2 with `actor=app`.
- Event model: webhook-driven by default (polling only as degraded fallback).
- Security model: signed webhook validation + replay window + delivery idempotency.
- Completion model: `Done` in Linear only after post-merge evidence confirms gate success.
- Compatibility model: canon `workspace.manifest` remains unchanged; Linear runtime details live under `x_*` keys.

## 3. Manifest Contract

Code247 keeps canonical keys in `inputs.linear` and adds runtime details through `x_*`.

Required manifest shape for Linear mode:

- `inputs.primary = "linear"`
- `inputs.linear` (canonical plugin contract)
- `x_linear_oauth` (OAuth app runtime)
- `x_linear_webhook` (signature/idempotency/runtime)
- `x_linear_automation` (lock/state/comment contract)

Schema for `x_*` extension:
- `schemas/code247-linear-runtime.extensions.schema.json`

## 4. OAuth2 Flow (actor=app)

### 4.1 Authorization

Endpoint (Linear):
- `https://linear.app/oauth/authorize`

Code247 sends:
- `client_id`
- `redirect_uri`
- `response_type=code`
- `state` (anti-CSRF)
- `scope` (least privilege)
- `actor=app`

### 4.2 Token Exchange

Endpoint (Linear):
- `https://api.linear.app/oauth/token`

Code247 exchanges `code` for:
- `access_token`
- `refresh_token`
- `expires_in`

### 4.3 Refresh

Code247 must refresh proactively before expiration and rotate stored tokens atomically.

## 5. Recommended Scopes

Minimum practical scope set:
- `read`
- `write`
- `comments:create`
- `issues:create` (optional if Code247 never creates issues)

Avoid:
- `admin` unless webhook lifecycle must be managed programmatically.

## 6. Webhook Contract

## 6.1 Inbound Endpoint

- `POST /webhooks/linear`

## 6.2 Validation

Hard requirements:
- Validate `Linear-Signature` with HMAC-SHA256 over raw body.
- Enforce replay window using `webhookTimestamp` (default `<= 60s`).
- Compare signatures in constant-time.

## 6.3 Delivery Idempotency

- Use `Linear-Delivery` header as idempotency key.
- Persist every delivery id with processing status.
- If already processed/accepted, return `200` and skip business execution.

## 6.4 Ack Strategy

- Webhook handler does only: verify -> dedupe -> enqueue.
- Return `200` in under 5 seconds.
- Worker executes heavy logic asynchronously.

## 7. Runtime State Contract

## 7.1 Claim/Lock Protocol

Trigger examples:
- Issue moved to execution-ready status.
- Issue receives queue label.

Lock sequence:
1. Add label `code247:locked`.
2. Comment: `Code247 run started: run_id=<id>`.
3. Persist `issue_id -> run_id` lock.

If duplicate event or locked issue: no-op.

## 7.2 Automation Labels

Suggested labels:
- `code247:queue`
- `code247:locked`
- `code247:running`
- `code247:needs-cloud-review`
- `code247:validated`

## 7.3 Human Checkpoints

When policy checkpoint is reached:
- Add label `code247:needs-cloud-review`.
- Post rationale + required action in comment.
- Trigger cloud re-evaluation path and resume automatically from gate result.

## 7.4 Completion Rule

Code247 can open and merge PRs automatically, but only transitions Linear issue to `Done` after:
- Merge confirmed.
- Post-merge CI/gates confirmed.
- Evidence comment published.

## 8. Required Evidence Comment

Before `Done`, Code247 posts:
- Plan summary.
- PR link.
- Gate results (`tests`, `types`, `security`, etc.).
- `how_to_test` and acceptance evidence.
- Run metadata (`run_id`, timestamps, route).

## 9. Operational Data Model

Required persisted entities:

- `linear_oauth_tokens`
  - `workspace_id`, `access_token`, `refresh_token`, `expires_at`, `scopes`, `updated_at`

- `linear_webhook_deliveries`
  - `delivery_id`, `event_type`, `received_at`, `status`, `error_message`

- `linear_issue_runs`
  - `issue_id`, `run_id`, `state`, `locked_by`, `started_at`, `ended_at`

## 10. Reliability Rules

- Worker retries with backoff for transient failures.
- DLQ for non-recoverable webhook jobs.
- Manual replay command for DLQ events.
- Lock TTL + stale lock recovery policy.

## 11. Observability

Minimum telemetry:
- Webhook ingest success/failure/latency.
- Signature/replay rejection counters.
- Delivery dedupe hit rate.
- Run lifecycle metrics by state.
- `Done` transitions blocked by missing evidence.

## 12. Security Requirements

- Secrets only via env/secret manager.
- No token or signature material in logs.
- OAuth state nonce and callback validation mandatory.
- Rotate webhook secret and OAuth client secret per runbook.

## 13. Compatibility with Canon 0.1

- Canonical `inputs.linear` remains source of truth for provider and claim baseline.
- Runtime-specific concerns (`oauth`, `webhook`, `automation`) are carried in `x_*` keys.
- Canon policy already permits `x_*`, so no breaking change is needed.
