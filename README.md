# LogLine Integration

Monorepo do ecossistema LogLine — infraestrutura de automação, billing e governança para desenvolvimento autônomo.

## Serviços

| Serviço | Descrição | Stack |
|---------|-----------|-------|
| [logic.logline.world](logic.logline.world/) | CLI + Core Crates (HQ) | Rust workspace |
| [llm-gateway.logline.world](llm-gateway.logline.world/) | Roteamento LLM + Billing | Rust |
| [code247.logline.world](code247.logline.world/) | Agente de Coding Autônomo | Rust |
| [edge-control.logline.world](edge-control.logline.world/) | Control Plane (policy, orchestration) | Rust |
| [obs-api.logline.world](obs-api.logline.world/) | Observability Dashboard | Next.js |

## Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                         Operators / UI                          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        edge-control                             │
│              (policy gates, orchestration, transistor)          │
└─────────────────────────────────────────────────────────────────┘
                                │
            ┌───────────────────┼───────────────────┐
            ▼                   ▼                   ▼
    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
    │  llm-gateway │    │    code247   │    │   obs-api    │
    │  (LLM proxy) │    │  (auto-code) │    │  (dashboard) │
    └──────────────┘    └──────────────┘    └──────────────┘
            │                   │                   │
            └───────────────────┼───────────────────┘
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Supabase                               │
│           (Postgres, Auth, Realtime, Storage)                   │
└─────────────────────────────────────────────────────────────────┘
```

## Princípios

1. **Rust owns business logic** — CLI e serviços são autoridade
2. **Supabase owns state** — única fonte de persistência
3. **Contract-first** — OpenAPI, JSON schemas, events registry
4. **Fuel is the common currency** — rastreia uso, custo e saúde operacional

## Quick Start

```bash
# Verificação operacional unificada
./scripts/verify-operations.sh

# Validar contratos
./scripts/validate-contracts.sh

# Smoke test de integração
./scripts/smoke.sh

# Ver serviços PM2
pm2 status
```

## Documentação Principal

| Documento | Descrição |
|-----------|-----------|
| [LLM_START_HERE.md](LLM_START_HERE.md) | Guia para agentes LLM |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Arquitetura e contratos |
| [SERVICE_TOPOLOGY.md](SERVICE_TOPOLOGY.md) | Rede, portas, DNS |
| [FUEL_SYSTEM_SPEC.md](FUEL_SYSTEM_SPEC.md) | Sistema Fuel (3 camadas) |
| [TASKLIST-GERAL.md](TASKLIST-GERAL.md) | Backlog atual |

## Operação

- `./scripts/verify-operations.sh`: entrypoint oficial para `smoke + sync-canon check + cookbook + contracts + integration-severe`, gerando:
  - `artifacts/operations-verify-report.json`
  - `artifacts/operations-verify-report.md`
- `./scripts/security-scan.sh`: varredura oficial de supply chain do monorepo; executa `cargo audit` em todos os `Cargo.lock` e `npm audit` nos `package-lock.json`, gravando `artifacts/security-scan-report.txt`.
- `./scripts/smoke-code247-stage-lease.sh`: smoke oficial de previsibilidade sob falha do `Code247`; injeta um job sintético com lease vencida e exige transição fail-closed para `FAILED` com evento único `lease_expired`.
- `./scripts/verify-code247-state-governance.sh`: prova oficial de governança de estado do `Code247`, cobrindo via HTTP os caminhos `In Progress -> Ready for Release`, bloqueio de `In Progress -> Done` e `Ready for Release -> Done`.
- `./scripts/smoke-obs-api-auth.sh`: smoke autenticado do `obs-api` para Fuel (`dashboard`, `alerts`, `calibration`, `reconciliation`, `ops`) com JWT de escopo real e relatório em `artifacts/obs-api-auth-smoke-report.json`. Rodar com `doppler run -- ./scripts/smoke-obs-api-auth.sh`.

## Fuel System

Fuel é a moeda comum do ecossistema:

- **Layer A**: Measurement Ledger (fatos brutos, append-only)
- **Layer B**: Valuation Ledger (USD estimated/settled, energia)
- **Layer C**: Control Currency (Fuel Points para decisões)

Ver [FUEL_SYSTEM_SPEC.md](FUEL_SYSTEM_SPEC.md) para detalhes.

## Estrutura

```
Integration/
├── logic.logline.world/      # CLI + crates
├── llm-gateway.logline.world/# LLM routing
├── code247.logline.world/    # Autonomous coding
├── edge-control.logline.world/# Control plane
├── obs-api.logline.world/    # Observability
├── contracts/                # Schemas, OpenAPI, events
├── policy/                   # Policy sets
├── scripts/                  # Operational scripts
└── LIXO/                     # Docs deprecated (review before delete)
```

## Secrets

Secrets via Doppler (`doppler run`) ou macOS Keychain via CLI (`logline secrets`).

**Nunca** commitar secrets. Ver [SECRETS_DOPPLER_RUNBOOK.md](SECRETS_DOPPLER_RUNBOOK.md).

## License

Proprietary — LogLine / VoulezVous
