# Governance

Este documento define como decisões são tomadas no ecossistema LogLine.

## Princípio Central: Transistor Pattern

No LogLine, **LLMs não executam diretamente** — eles emitem opiniões que são julgadas deterministicamente por código Rust.

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│   LLM Agent     │ ───► │  edge-control   │ ───► │    Execution    │
│  (emite opinião)│      │  (julga regras) │      │  (se aprovado)  │
└─────────────────┘      └─────────────────┘      └─────────────────┘
     OpinionSignal            GateDecision           Action/Block
```

**Por quê?**
- LLMs são probabilísticos; decisões críticas precisam ser determinísticas
- Auditabilidade: toda decisão tem `reason_codes` e `trace_id`
- Fail-closed: na dúvida, bloqueia

## Hierarquia de Autoridade

| Camada | Autoridade | O que decide |
|--------|------------|--------------|
| **Rust** | Domínio | Regras de negócio, validações, cálculos |
| **Supabase** | Estado | Dados canônicos, persistência, auth |
| **edge-control** | Governança | Gates, policy enforcement, orchestration |
| **obs-api** | Observação | Visualização (nunca decide) |
| **LLM** | Opinião | Sugestões, drafts, análises (nunca executa) |

## Checkpoints Humanos Obrigatórios

Dois pontos exigem aprovação humana explícita:

| Checkpoint | Quando | Quem aprova |
|------------|--------|-------------|
| **YES/HUMAN #1** | Confirmar `DraftIntention` antes de execução | Operator |
| **YES/HUMAN #2** | Aprovar rollback/undo/ação destrutiva | Operator |

**Não existe bypass programático** para esses checkpoints.

## Policy-Driven Decisions

Decisões são governadas por `policy/policy-set.v1.1.json`:

```json
{
  "defaults": {
    "mode": "NO_HUMAN_MIDDLE",
    "fail_closed": true,
    "allowed_decisions": ["YES", "NO", "CLOUD"]
  }
}
```

| Decision | Significado |
|----------|-------------|
| `YES` | Aprovado automaticamente |
| `NO` | Bloqueado (com reason_code) |
| `CLOUD` | Escalado para revisão humana ou sistema externo |

### Domínios Sensíveis (sempre CLOUD)

Qualquer mudança em:
- `auth/`, `billing/`, `permissions/`
- `migrations/`, `infra/`
- `.github/workflows/`

É automaticamente escalada para revisão.

## Code247: Agente Autônomo

O agente Code247 opera com autonomia limitada:

| Ação | Autonomia |
|------|-----------|
| Criar branch, escrever código | ✅ Autônomo |
| Abrir PR | ✅ Autônomo (com evidências obrigatórias) |
| Merge `light` risk | ✅ Auto-merge (após CI verde) |
| Merge `substantial` risk | ⚠️ Requer gates adicionais |
| Marcar `Done` no Linear | ❌ Bloqueado sem evidência de deploy |
| Rollback/undo | ❌ Requer YES/HUMAN #2 |

### Risk Classification

PRs são classificados deterministicamente:

- `+3`: toca auth/billing/permissions/migrations/infra
- `+2`: muda API/contrato ou diff > 200 linhas
- `+1`: concorrência/performance/caches

**Score ≥ 3 = Substantial** (gates mais rigorosos)

## CLI First

Toda capacidade deve existir no CLI (`logline`) antes de ser exposta via API ou UI.

```
Capability: CLI → API → UI
```

**Por quê?**
- CLI é auditável e scriptável
- Touch ID como hardware root of trust
- Nenhum secret em disco

## Contribuições

### Para código comum
1. Branch `codex/<agent-id>-<topic>`
2. PR com evidências (plan, AC, risk, backout)
3. CI verde
4. Review de CODEOWNER da área

### Para contratos (`contracts/*`, `policy/*`, `openapi/*`)
1. Tudo acima +
2. Executar `./scripts/validate-contracts.sh`
3. Review de quem mantém edge-control

### Para mudanças de governança
1. Proposta documentada com justificativa
2. Revisão por maintainers
3. Atualização deste documento

## Decisões Registradas

| Data | Decisão | Justificativa |
|------|---------|---------------|
| 2026-03 | Transistor Pattern adotado | LLMs não devem executar diretamente |
| 2026-03 | Supabase como único backend | Simplifica auth, RLS, realtime |
| 2026-03 | Fuel como moeda comum | Unifica billing, observability, controle |
| 2026-03 | UI por último | Backend/contracts/gates primeiro |

## Versionamento de Policies

- Toda policy tem `version` no JSON
- `price_card_version` acompanha mudanças de pricing
- `valuation_version` acompanha mudanças de cálculo Fuel
- Mudanças são append (histórico preservado)
