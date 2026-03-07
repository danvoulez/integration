# LLM START HERE

## Scope

- Workspace: `Integration`
- Primary stack: Rust services + Next.js `obs-api`
- Primary infra: Supabase
- Primary secret source: `Doppler`

## Read Order

1. This file
2. Service-local `LLM_START_HERE.md`
3. `TASKLIST-GERAL.md`
4. Only then open broader docs

## Service Map

- `logic.logline.world`: CLI + shared Rust crates
- `llm-gateway.logline.world`: canonical LLM routing + fuel/accounting
- `code247.logline.world`: autonomous coding pipeline + Linear + GitHub
- `edge-control.logline.world`: policy/control-plane + orchestration
- `obs-api.logline.world`: observability/read models only

## Global Invariants

- Supabase is first-class infra. Do not treat it as optional integration.
- Secrets come from `doppler run --project logline-ecosystem --config <env> -- ...`
- `.env` is fallback for isolated local dev only.
- Do not hardcode secrets, tokens, URLs with credentials, or connection strings.
- Do not bypass contracts, policy, or canonical schemas.
- Do not invent mock data in production paths.
- Do not mark Linear `Done` without required evidence.
- Do not emit cross-service events without `request_id`.

## Operational Defaults

- Prefer backend work over UI.
- Prefer existing scripts over ad hoc commands.
- Prefer Rust services as authority; `obs-api` observes, not decides.
- Prefer `rg`, targeted tests, and narrow diffs.

## Canonical Commands

```bash
./scripts/validate-contracts.sh
./scripts/security-scan.sh
./scripts/verify-operations.sh
doppler run --project logline-ecosystem --config dev -- ./scripts/smoke-obs-api-auth.sh
```

## When Editing

- If touching `contracts/*`, `policy/*`, or OpenAPI: run contract validation.
- If touching auth/service-to-service: assume `Doppler` and Supabase JWT are the norm.
- If touching Supabase-backed behavior: think in terms of remote project, migrations, RLS, Realtime.
- If touching docs for agents: optimize for minimal tokens, imperative wording, no prose.

## Next File

- Open the service-local `LLM_START_HERE.md` for the service you will edit.
