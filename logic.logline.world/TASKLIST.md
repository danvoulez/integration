# Logic Workspace Tasklist

**Updated:** 2026-03-05

## Integration-critical tasks
- [ ] Harden auth policies (SEC- and HARD- items from integration tasklist).
- [ ] Ensure `logline-supabase` crate publishes schema updates used by code247 + llm-gateway.
- [ ] Keep `schemas/workspace.manifest.*.json` in sync with canonical AST and generate TypeScript from Rust (`ENF-002`).
- [x] Add canonical contracts validator (`contracts_validate`) for schemas/registry/policy/openapi and use it in CI.
- [x] Mirror runtime checkpoints (`status/run/stop/events`) to `obs-api` ingest when `OBS_API_BASE_URL` is configured.

## Linear intent sync
- [ ] Produce `manifest.intentions.json` describing readiness gates + policy upgrades, and POST it to `/intentions` after each commit.
- [ ] Record the returned Linear issue IDs (`.code247/linear-meta.json`) for traceability.
- [ ] Maintain the aggregated runbook (`docs/runbook.md`) referencing new privacy/transition guardrails from `docs/privacy-safety.md`.
