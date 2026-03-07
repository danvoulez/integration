# LLM START HERE: LOGIC

## Authority

- Shared Rust workspace for CLI, auth, runtime, connectors, catalog
- Owns harness/verification backbone for the ecosystem

## Entry Paths

- `crates/logline-cli/`: operational CLI
- `crates/logline-auth/`: JWT/JWKS helpers
- `crates/logline-core/`: policy/catalog/domain
- `crates/logline-runtime/`: runtime orchestration
- `supabase/migrations/`: canonical SQL changes

## Operational Priority

- Prefer extending CLI/harness over adding ad hoc scripts
- Prefer migrations in `supabase/migrations` over dashboard-only changes
- Prefer canonical shared auth/helpers here instead of duplicating per service

## Secrets

- This workspace interacts with real infra
- Use `doppler run --project logline-ecosystem --config <env> -- <command>` when secrets are needed
- Do not hardcode tokens or connection strings

## Canonical Commands

```bash
cargo check --manifest-path logic.logline.world/Cargo.toml --workspace
cargo run --manifest-path logic.logline.world/Cargo.toml -p logline-cli -- harness run --root /Users/ubl-ops/Integration
/Users/ubl-ops/Integration/scripts/verify-operations.sh
```

## Migration Rules

- Every DB change is a migration file
- Supabase is primary infra; treat migrations as first-class deliverables
- Do not make remote-only dashboard edits without matching migration/state in repo

## Do Not Do

- Do not fork shared auth logic into services without need
- Do not bypass harness when a verification path belongs in CLI
- Do not leave operational scripts undocumented if they should be CLI/harness features

## Next Docs

- `../README.md`
- `/Users/ubl-ops/Integration/TASKLIST-GERAL.md`
