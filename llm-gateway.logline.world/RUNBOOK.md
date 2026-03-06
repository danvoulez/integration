# LLM Gateway Runbook

## Routine checks
- `cargo check` → gate0 contract validation + manifest requirements.  
- `cargo test` → ensures runtime routines (add more commands as new features appear).  
- `cargo fmt --check` and `cargo clippy --all-targets` prior to release.
- Validate manifest: `.code247/workspace.manifest.json` must exist and follow the canonical schema; refer to `schemas/workspace.manifest.schema.json` and `.code247/` in CI.

## Deployment steps
1. Rebuild: `cargo build --release`.  
2. Refresh `openapi.yaml` (if routes change) and ensure `contracts.openapi_paths` still references it.  
3. Package via `ecosystem.config.cjs` or whichever deployment script touches `gateway.log`.

## Incident procedures
- If `gateway.log` reports schema/contract mismatch, rerun `cargo run --bin validate_manifest` (or `cargo check` plus manifest regen).  
- For failing Linear inputs, update `.code247/workspace.manifest.json` (inputs.linear section) referencing the canonical plugin schema and rerun `cargo check`.
- Deploy new version only after gates (cargo check/test) and manifest validation succeed.

## Integration notes
- Manifest path `.code247/workspace.manifest.json` is tracked and validated by Code247 tooling; keep credentials (LINEAR_API_KEY) referenced in README.  
- Share runbook updates with other apps via `logline.world` coordination to ensure consistent gates.
