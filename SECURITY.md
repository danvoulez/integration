# Security Policy

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

Send vulnerability reports to: **security@logline.world**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You will receive a response within **48 hours** acknowledging receipt. We will work with you to understand the issue and coordinate disclosure.

---

## Security Model

### 1) Principle: CLI or Nothing

All infrastructure operations MUST use the `logline` CLI. This is enforced by design:

- No `DATABASE_URL` exists anywhere on disk
- No API tokens exist anywhere on disk
- All secrets live in macOS Keychain, requiring Touch ID
- Supabase PostgREST requires a valid JWT that only exists in Keychain

**Implication:** Attempts to bypass the CLI will fail. This is intentional.

### 2) Secret Handling

| Rule | Description |
|------|-------------|
| Never commit secrets | No tokens, keys, or connection strings in git |
| No `.env` with secrets | `.env*` is for public runtime values only |
| Production secrets | Managed via Doppler (`project=logline-ecosystem`) |
| Rotation | Compromised keys are rotated immediately |

**Sensitive values:**
- `SUPABASE_JWT_SECRET`
- `SUPABASE_SERVICE_ROLE_KEY`
- `SUPABASE_ACCOUNT_TOKEN` / `SUPABASE_ACCESS_TOKEN`
- Provider API keys (OpenAI, Anthropic, Linear, GitHub)

### 3) Authentication

#### Service-to-Service
- JWT validation via Supabase JWKS
- Required claims: `tenant_id`, `app_id`, `user_id`
- Scopes enforce permission boundaries
- Cross-tenant access is **denied by default**

#### Human Operators
- Touch ID biometric required for session unlock
- 30-minute session TTL
- Passkey authentication to Supabase Auth

### 4) Sensitive Paths

The following paths require elevated review (substantial classification):

```
**/auth/**
**/billing/**
**/permissions/**
**/migrations/**
**/infra/**
.github/workflows/**
Cargo.lock
package-lock.json
```

PRs touching these paths:
- Cannot auto-merge as "light"
- Require security scan pass
- Trigger supply-chain guardrails workflow

Repository guardrails for `main` are prepared to run on both `pull_request` and
`merge_group`. After the updated workflows reach the default branch, enable the
GitHub ruleset/merge queue so these scans become blocking instead of advisory.

### 5) Code247 Security Controls

The autonomous coding agent has explicit guardrails:

| Control | Description |
|---------|-------------|
| Runner allowlist | Only commands in `gates.gate0.commands` can execute |
| Fail-closed | Unknown commands are blocked, not allowed |
| Evidence required | No `Done` without CI/deploy proof |
| Human checkpoints | Destructive operations require YES/HUMAN approval |

### 6) Transistor Pattern

LLM output is never executed directly:

1. LLM emits `OpinionSignal.v1` (suggestion only)
2. Rust judge applies deterministic rules
3. `GateDecision.v1` is emitted with audit trail
4. Unknown or malformed LLM output **fails closed**

### 7) Incident Response

If a security incident is suspected:

1. **Revoke** compromised credentials immediately
2. **Rotate** via `logline secrets set` or Doppler
3. **Restart** affected services (`pm2 restart`)
4. **Audit** access logs in Supabase and fuel_events
5. **Report** to security@logline.world

### 8) Dependencies

- Dependabot enabled for security advisories
- Supply-chain guardrails block PRs modifying dependencies without review
- Lock files (`Cargo.lock`, `package-lock.json`) are tracked and reviewed

---

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | ✅ Active |
| Feature branches | ⚠️ Best effort |
| Archived releases | ❌ No support |

---

## Security Tools

```bash
# Validate secret chain integrity
logline secrets doctor

# Check for dependency vulnerabilities
./scripts/security-scan.sh     # Rust + Node.js lockfiles
cargo audit                    # Rust (low-level)
npm audit                      # Node.js (low-level)

# Verify RLS policies
logline db verify-rls
```

---

## Acknowledgments

We appreciate responsible disclosure and will acknowledge researchers who report valid vulnerabilities (unless they prefer to remain anonymous).
