# Logic API Contracts

This document defines the API surface exposed by `logic.logline.world` crates.

## 1) Overview

`logic.logline.world` is primarily a CLI workspace. It does not expose HTTP APIs directly. Instead, it provides:

1. **CLI commands** — operator interface
2. **Crate APIs** — Rust trait contracts for other services
3. **Supabase operations** — via `logline-supabase` crate

## 2) CLI Command Surface

### Auth Commands

```bash
logline auth unlock              # Start Touch ID session
logline auth login --passkey     # Login via passkey
logline auth login --email <e>   # Login via email
logline auth lock                # End session
logline auth status              # Check session state
```

### Secrets Commands

```bash
logline secrets set <key>        # Store in Keychain
logline secrets get <key>        # Retrieve value
logline secrets doctor           # Full health check
```

### Database Commands

```bash
logline db tables                # List tables
logline db query "SELECT ..."    # Execute SQL
logline db verify-rls            # Verify RLS policies
logline migrate review           # Review pending migrations
logline migrate apply --env prod # Apply migrations
```

### Deploy Commands

```bash
logline deploy all --env prod    # Full deploy
logline cicd run --pipeline prod # Run CI/CD pipeline
logline ready --pipeline prod    # Pre-flight check
```

### Harness Commands (Code247 Integration)

```bash
logline harness intentions generate --root ..  # Generate manifest
logline harness intentions publish --root ..   # Publish to Code247
logline harness run --root ..                  # Full integration run
```

## 3) Crate Trait Contracts

### logline-auth

```rust
pub trait TokenValidator {
    fn validate(&self, token: &str) -> Result<Claims, AuthError>;
}

pub trait TenantResolver {
    fn resolve(&self, claims: &Claims) -> Result<Tenant, AuthError>;
}
```

### logline-supabase

```rust
pub trait SupabaseClient {
    async fn query<T>(&self, table: &str, query: Query) -> Result<Vec<T>>;
    async fn insert<T>(&self, table: &str, data: &T) -> Result<()>;
    async fn emit_fuel_event(&self, event: FuelEvent) -> Result<()>;
}
```

### logline-core

```rust
pub trait PolicyEvaluator {
    fn evaluate(&self, context: &PolicyContext) -> PolicyDecision;
}
```

## 4) Supabase Tables (via logline-supabase)

| Table | Purpose |
|-------|---------|
| `users` | User accounts |
| `tenants` | Organizations |
| `apps` | Applications |
| `fuel_events` | Usage ledger |
| `fuel_valuations` | Cost valuations |

## 5) Output Contracts

CLI outputs use consistent shapes for automation:

```bash
logline --json <command>   # JSON output mode
```

All JSON outputs include:
- `ok: bool`
- `data: T` (on success)
- `error: { code, message }` (on failure)
