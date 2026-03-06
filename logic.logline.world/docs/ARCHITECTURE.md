# Logic Architecture

This document defines the architecture of `logic.logline.world` — the CLI + core crates workspace.

## 1) Crate Structure

```
logic.logline.world/
├── crates/
│   ├── logline-api/        # Shared models + trait contracts
│   ├── logline-auth/       # JWT/JWKS verification + tenant/cookie helpers
│   ├── logline-core/       # Domain policy + catalog validation
│   ├── logline-connectors/ # Connector implementations/factory
│   ├── logline-runtime/    # Runtime engine orchestration
│   ├── logline-cli/        # The CLI binary
│   └── logline-supabase/   # Supabase client (PostgREST, Realtime, Storage)
├── supabase/
│   └── migrations/         # SQL migrations
├── schemas/                # JSON schemas for manifests
└── docs/                   # Documentation
```

## 2) Design Principles

1. **CLI is the first interface** — all capabilities must exist in CLI before API exposure
2. **Supabase is the sole infrastructure** — no alternative persistence paths
3. **Touch ID is the hardware root of trust** — secrets live in macOS Keychain
4. **No secrets on disk** — ever

## 3) Crate Responsibilities

### logline-cli
- The only binary that matters
- Entry point for all operator commands
- Handles Touch ID unlock, passkey auth, secrets management

### logline-auth
- JWT validation (Supabase JWKS)
- Tenant resolution from claims
- Session cookie helpers

### logline-core
- Domain policy definitions
- Catalog validation
- Business rules

### logline-supabase
- PostgREST client
- Realtime subscriptions
- Storage operations
- Fuel event emission

### logline-api
- Shared types and models
- Trait definitions for cross-crate contracts

### logline-connectors
- External service integrations
- Factory pattern for connector instantiation

### logline-runtime
- Runtime orchestration
- Pipeline execution

## 4) Data Flow

```
Operator → CLI Command → Touch ID → Keychain → Supabase
                              ↓
                        Session (30m)
```

## 5) Key Commands

```bash
logline auth unlock              # Touch ID session
logline auth login --passkey     # Supabase Auth via passkey
logline secrets doctor           # Full health check
logline ready --pipeline prod    # Pre-flight readiness
logline deploy all --env prod    # Full deploy
logline harness intentions generate  # Generate manifest.intentions.json
logline harness intentions publish   # Publish to Code247
```

## 6) Integration Points

| Target | Method |
|--------|--------|
| Supabase | PostgREST + Realtime + Storage via `logline-supabase` |
| Code247 | `POST /intentions` via `harness intentions publish` |
| obs-api | Event mirror via `OBS_API_BASE_URL` |
