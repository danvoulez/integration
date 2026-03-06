# LLM Gateway Blueprint

## Mission
Provide the canonical gateway/app experience for the logline.world ecosystem: expose the OpenAPI contract, host gateway-specific logic, and remain interoperable with the shared canon and manifests.

## Objectives
1. Keep `openapi.yaml` and generated schemas in lock-step with the canonical AST (`canon/workspace-ast/0.1`).  
2. Maintain manifest-driven engineering guardrails (`.code247/workspace.manifest.json` + `schemas/workspace.manifest*.json`).  
3. Provide deployment/readiness entry points (build/test commands, logs, config) that other apps can rely on.

## Success criteria
- Manifest validation passes (`cargo check` gate, `schemas` folder).  
- OpenAPI spec is up-to-date with runtime routes and referenced from `contracts.openapi_paths`.  
- README/runbook/blueprint exist and contain deployment/validation steps (this file plus `RUNBOOK.md` ensures the “mínimo conciso”).

## Dependencies
- Canon schemas: `canon/workspace-ast/0.1` (permissive/strict manifests + plugin schema).  
- Linear task input: `schemas/inputs.linear.schema.json`.  
- Deployment config: `ecosystem.config.cjs`, `gateway.log`.

## Communication
- Share readiness via `manifest` gating commands and keep this blueprint referenced in PRs.  
