# Edge Control — LLM Start Here

**Read the root `LLM_START_HERE.md` first.**

## What is Edge Control?

Edge Control is the control-plane service for the LogLine ecosystem. It enforces policy, orchestrates workflows, and acts as the "judge" in the Transistor pattern.

## Architecture

```
Request → Auth → Policy Gate → Execution → Fuel Event → Response
                    ↓
           policy-set.v1.1.json
```

## Transistor Pattern

1. **LLM emits `OpinionSignal.v1`** — draft intention, risk opinion
2. **Edge Control applies deterministic rules** — policy-set.v1.1
3. **Edge Control emits `GateDecision.v1`** — allow/deny with reason

**No direct execution from LLM output is allowed.**

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point |
| `src/auth.rs` | JWT/token validation |
| `src/policy.rs` | Policy loading and evaluation |
| `src/handlers.rs` | Route handlers |
| `src/orchestration.rs` | Workflow orchestration |
| `src/fuel.rs` | Fuel event emission |

## Critical Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /v1/intention/draft` | Draft intention (LLM → opinion) |
| `POST /v1/pr/risk` | PR risk assessment |
| `POST /v1/fuel/diff/route` | Fuel diff routing |
| `POST /v1/orchestrate/intention-confirmed` | YES/HUMAN #1 checkpoint |
| `POST /v1/orchestrate/rollback` | YES/HUMAN #2 checkpoint |
| `POST /v1/orchestrate/github-event` | GitHub → Linear sync |
| `GET /health` | Health check |

## Human Checkpoints

| Checkpoint | When |
|------------|------|
| **YES/HUMAN #1** | Confirm `DraftIntention` before execution |
| **YES/HUMAN #2** | Approve rollback/undo/destructive reversal |

## Policy Enforcement

- Default policy: `../policy/policy-set.v1.1.json`
- Override: `EDGE_CONTROL_POLICY_SET_PATH`
- Every protected endpoint emits:
  - `x-gate-decision` header
  - `x-gate-policy` header
  - Structured log entry

## What You MUST NOT Do

1. **Never bypass policy gates**
2. **Never emit `GateDecision` without causal `trace_id` and `parent_event_id`**
3. **Never approve human checkpoints programmatically**
4. **Never modify policy without validation**

## What You SHOULD Do

1. Always emit causal events (`trace_id`, `parent_event_id`)
2. Include `reason_codes` in gate decisions
3. Mirror events to `obs-api`
4. Use deterministic policy evaluation

## Quick Commands

```bash
# Run locally
cargo run

# Health check
curl http://localhost:8080/health
```

## Key Docs

- `README.md` — Endpoints and configuration
- `TASKLIST.md` — Current backlog
- `../policy/policy-set.v1.1.json` — Active policy
- `../contracts/schemas/gate-decision.v1.schema.json` — Gate decision contract
