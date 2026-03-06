# Code247 — LLM Start Here

**Read the root `LLM_START_HERE.md` first.**

## What is Code247?

Code247 is the autonomous coding agent of the LogLine ecosystem. It receives intentions, converts them to Linear issues, executes code changes, and manages the full CI/CD cycle.

## Architecture

```
Intentions → Linear Issue → Job → Plan → Code → Review → PR → CI → Merge → Deploy
```

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point |
| `src/state_machine_rs.rs` | Linear state transitions |
| `src/pipeline_rs.rs` | Execution pipeline |
| `src/policy_gate_rs.rs` | Policy enforcement |
| `src/risk_classifier_rs.rs` | PR risk scoring |
| `.code247/workspace.manifest.json` | Project manifest |

## Critical Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /intentions` | Intake from ecosystem |
| `POST /intentions/sync` | Sync execution state to Linear |
| `GET /intentions/{workspace}/{project}` | Get linkage snapshot |
| `GET /jobs` | List jobs (requires auth) |
| `GET /health` | Health check |

## State Machine Rules

**Allowed transitions:**
- `Ready` → `In Progress (Code247)`
- `In Progress` → `Ready for Release`
- `Ready for Release` → `Done`

**Forbidden (will be blocked):**
- `Ready` → `Done` (skips gates)
- `In Progress` → `Done` (requires CI evidence)
- Any state → `Done` without evidence

## Evidence Requirements

| State | Required Evidence |
|-------|-------------------|
| `Ready for Release` | CI passed (`ci.url`) |
| `Done` | Deploy completed (`deployment_id` or `deploy.url`) |

## Risk Classification

PR risk is scored deterministically:
- `+3`: touches auth/billing/permissions/migrations/infra
- `+2`: changes API/contract or diff > 200 lines
- `+1`: concurrency/performance/caches

**Score ≥ 3 = Substantial** (stricter gates, no auto-merge)

## What You MUST NOT Do

1. **Never mark `Done` without deploy evidence**
2. **Never bypass the runner allowlist** (`gates.gate0.commands`)
3. **Never push directly to main** — always via PR
4. **Never skip risk classification**

## What You SHOULD Do

1. Always emit `plan_contract`, `acceptance`, `risk`, `backout` before PR
2. Use deterministic risk scoring
3. Emit events to `obs-api` via `OBS_API_BASE_URL`
4. Persist state to Supabase (`code247_jobs`, `code247_events`)

## Quick Commands

```bash
# Run locally
cargo run

# Validate manifest
cargo run --bin validate_manifest

# Smoke test state governance
./scripts/smoke-p1-state-governance.sh
```

## Key Docs

- `README.md` — Full endpoint and env reference
- `docs/Code247_Ciclo_Completo_Operacao_v1.1.md` — Complete operational cycle
- `docs/Code247-Linear-Integration.md` — Linear contract
- `TASKLIST.md` — Current backlog
