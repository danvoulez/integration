# Linear Setup Runbook for Code247

## 1. Linear Admin Setup

1. Create OAuth app in Linear admin (`Settings -> Administration -> API`).
2. Configure callback URL(s):
   - `https://code247.<domain>/oauth/callback`
3. Keep `actor=app` in authorization requests.
4. Configure scopes:
   - `read`, `write`, `comments:create`
   - Add `issues:create` only if needed.
5. Create webhook in Linear UI (recommended internal mode):
   - Target URL: `https://code247.<domain>/webhooks/linear`
   - Resource types: `Issue`, `Comment`.
6. Store webhook signing secret securely.

## 2. Code247 Secret Configuration

Set these environment variables:

- `LINEAR_CLIENT_ID`
- `LINEAR_CLIENT_SECRET`
- `LINEAR_OAUTH_REDIRECT_URI`
- `LINEAR_WEBHOOK_SIGNING_SECRET`
- `LINEAR_TOKEN_ENCRYPTION_KEY` (if token-at-rest encryption enabled)

Optional operational variables:

- `LINEAR_WEBHOOK_MAX_SKEW_MS` (default `60000`)
- `LINEAR_WEBHOOK_DEDUPE_TTL_HOURS` (default `168`)
- `LINEAR_WEBHOOK_ACK_TIMEOUT_MS` (default `5000`)

## 3. Manifest Configuration

Update `.code247/workspace.manifest.json` with:
- `inputs.primary = "linear"`
- `inputs.linear` canonical fields
- `x_linear_oauth`, `x_linear_webhook`, `x_linear_automation`

Validate with:

```bash
./scripts/validate-manifest.sh
./scripts/validate-linear-extensions.sh
```

## 4. OAuth Verification

1. Open `GET /oauth/start`.
2. Complete consent on Linear.
3. Confirm callback stores `access_token + refresh_token`.
4. Confirm background refresh runs before token expiry.

Success criteria:
- Token refresh succeeds without manual re-auth.
- No auth failures during steady-state polling/webhook execution.

## 5. Webhook Verification

1. Send test webhook from Linear.
2. Confirm handler validates signature and timestamp.
3. Confirm response returns `200` quickly.
4. Confirm event is enqueued and processed by worker.
5. Replay same delivery id and confirm dedupe no-op.

Success criteria:
- Signature mismatch -> rejected.
- Replay outside skew window -> rejected.
- Duplicate delivery -> accepted but ignored.

## 6. Runtime Flow Verification

1. Tag issue with `code247:queue` or move to ready status.
2. Confirm Code247 lock sequence:
   - adds `code247:locked`
   - posts run start comment
3. Confirm PR/gates execution.
4. Confirm evidence comment posted.
5. Confirm issue moves to `Done` only after evidence and gate pass.

## 7. Failure Modes and Recovery

- Webhook transient failure:
  - rely on Linear retries and worker retry policy.
- Worker non-recoverable failure:
  - move event to DLQ.
- Stale lock:
  - lock TTL expiry + recovery command (`unlock` runbook step).
- OAuth invalid grant:
  - revoke stored token and trigger re-auth flow.

## 8. Go-Live Gate

Production ready only when all are true:
- OAuth callback and refresh are stable.
- Webhook signature and dedupe are validated.
- Run lock/idempotency is validated.
- Done transition is blocked without evidence.
- Dashboards and alerts are active.
