# Logic Security Guide

This document defines the security model for `logic.logline.world`.

## 1) Core Security Model

**CLI or Nothing** — all infrastructure operations MUST use the `logline` CLI.

### Why You Cannot Bypass It

1. No `DATABASE_URL` exists anywhere on disk
2. No API tokens exist anywhere on disk
3. All secrets live in macOS Keychain, requiring Touch ID
4. `psql` without a connection string does nothing
5. Supabase PostgREST requires a valid JWT that only exists in Keychain

### What This Means

- NEVER attempt direct database connections
- NEVER create `.env` files with secrets
- NEVER hardcode tokens or connection strings
- When infrastructure access is needed, use CLI commands
- Human must approve via Touch ID — by design

## 2) Secret Handling Rules

1. Never commit live secrets/tokens
2. Use `.env*` for local runtime only (public values)
3. Production secrets go in Doppler (`project=logline-ecosystem`)
4. Rotate compromised keys immediately

### Sensitive Values

- `SUPABASE_JWT_SECRET`
- `SUPABASE_SERVICE_ROLE_KEY`
- `SUPABASE_ACCOUNT_TOKEN` / `SUPABASE_ACCESS_TOKEN`
- Provider API keys (OpenAI, Anthropic, Linear, GitHub)

## 3) Identity and Scope

### Resolution Order

1. JWT mode: `Authorization: Bearer <supabase_jwt>`
2. Claims: `tenant_id`, `app_id`, `user_id`
3. Scopes: permissions from JWT claims

### Scope Enforcement

Protected operations MUST resolve:
- `tenant_id` — billing organization
- `app_id` — source application
- `user_id` — acting user

Cross-tenant access is **denied by default**.

## 4) Auth Flow

```
logline auth unlock     # Touch ID → 30m session
logline auth login      # Passkey → Supabase JWT
logline <command>       # Uses session + JWT
```

### Session Rules

- Default TTL: 30 minutes
- Requires Touch ID biometric
- No session = no infrastructure access

## 5) Keychain Integration

All secrets stored using macOS Security framework:
- Service: `logline`
- Access: requires Touch ID or password

```bash
logline secrets set <key>     # Store in Keychain
logline secrets get <key>     # Retrieve (requires unlock)
logline secrets doctor        # Validate entire chain
```

## 6) Incident Response

If secret leakage suspected:
1. Revoke compromised keys immediately
2. Rotate via `logline secrets set`
3. Restart affected services
4. Audit access logs

## 7) CI/CD Security

- Service tokens have minimal scope
- Doppler injects secrets at runtime (`doppler run`)
- No secrets in PM2 ecosystem.config.cjs
- GitHub Actions use repository secrets
