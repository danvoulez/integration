# CODE247
## Ciclo Completo de Operacao
### Do intake de intentions ate codigo em producao

LogLine Foundation | Marzo 2026 | v1.0  
v1.1 (Hardening): adiciona SPEC/AC, risk scoring deterministico, backout, politica de CI flaky/red-main, guardrails de supply chain, regras de idempotencia/sync, e definicao operacional de Done.

---

## Visao Geral

Este documento define o ciclo operacional completo do Code247: como intentions viram codigo em producao com qualidade, seguranca e rastreabilidade. O objetivo e ter um processo que rode com autonomia maxima do agente nos casos de baixo risco, e envolva humanos apenas quando necessario.

Principio central: o agente nao recebe mais confianca que um dev junior. Ele recebe mais automacao de gates e precisa produzir mais evidencia.

Definicoes:
- "Evidencia" = checks verdes + descricao completa (plano/validacao) + artefatos (logs, screenshots, links de run CI, etc).
- "Fonte da verdade":
- Linear = estado do trabalho/decisao humana
- GitHub = estado do codigo/PR/merge
- CI/CD = estado de qualidade e entrega
- Code247 = orquestrador (nunca a verdade final)

---

## O Fluxo: 8 Passos

Cada passo tem owner, entradas, saidas e criterios de saida claros.

Nota de operacao:
- Todos os comandos/acoes do agente devem ser executados em ambiente controlado (runner) com allowlist de comandos.
- Eventos sao idempotentes: replays e duplicatas nao podem duplicar issue/job/PR.

## Workflow Oficial no Linear (estados e transicoes)

Para eliminar ambiguidade entre times, o fluxo operacional usa estes estados oficiais no Linear:

| Estado Linear | Dono principal | Significado operacional | Entra quando | Sai quando |
|---|---|---|---|---|
| Ready | Humano (triagem) | Issue pronta para execucao automatica | Intake completo (campos minimos + AC + nao-objetivos) e prioridade definida | Code247 faz claim |
| In Progress (Code247) | Code247 | Issue em execucao ativa (plan/code/review) | Claim concluido e job vinculado | PR mergeado ou bloqueio externo/humano |
| Ready for Release | CI/CD + Humano | Codigo mergeado, aguardando janela/release train | Merge concluido sem deploy imediato, com link rastreavel de release | Deploy concluido com evidencia |
| Done | CI/CD (e espelhado por Code247) | Trabalho entregue em producao com evidencia | Merge + deploy concluido + healthchecks/observabilidade ok | Estado terminal |

Regras de transicao permitidas:
- Ready -> In Progress (Code247)
- In Progress (Code247) -> Ready for Release
- Ready for Release -> Done

Transicoes proibidas:
- Ready -> Done (pula gates)
- In Progress (Code247) -> Done (mesmo com evidencia; deve passar por Ready for Release)
- Qualquer estado -> Done por acao manual sem links de evidencia

### 0) INTAKE
Owner: Ecossistema + Code247

- `/intentions` recebe pedidos de codigo de todo o ecossistema
- Code247 faz dedup, normaliza e sincroniza no Linear
- Toda intention vira issue com campos minimos: app, impacto, risco, definicao de pronto
- Regra: nenhum trabalho comeca sem issue no Linear

Campos obrigatorios adicionais (anti-retrabalho):
- Acceptance Criteria (3-5 bullets verificaveis; estilo "dado/quando/entao")
- Nao-objetivos (1-2 bullets) para limitar escopo
- Se for bug: passos de reproducao + expected vs actual

Criterio de saida:
- Issue criada e completa (campos minimos + AC/nao-objetivos).

### 1) CLAIM
Owner: Code247 (automatico)

- Code247 faz polling/stream de issues "Ready / Unclaimed" no Linear
- Ao clamar: cria job interno + grava link Issue <-> Job
- Seta estado no Linear para "In Progress (Code247)"
- Nenhum humano precisa intervir neste passo

Regras de idempotencia:
- Se issue ja estiver linkada a job ativo, nao cria outro job.
- Se Code247 cair e voltar, ele reanexa ao job existente.

Criterio de saida:
- Job criado (ou reanexado) + estado do Linear atualizado + lock de ownership registrado.

### 2) PLAN (inclui SPEC/AC como contrato)
Owner: Code247

- Gera plano curto e padronizado (comentario na issue + arquivo no repo)
- Campos obrigatorios: Mudancas (arquivos/areas), Estrategia (feature flag? rollout?), Validacao (testes, checks), Risco (low/med/high + motivo)
- Auto-classifica risco como HIGH quando detecta: migracao/db, auth, billing, infra, mudancas amplas, diffs grandes
- Plano e artefato auditavel - fica registrado na issue e no PR

Campos obrigatorios adicionais (hardening):
- Acceptance Criteria (copiado/normalizado do intake; vira contrato de validacao)
- Backout/Rollback: como desfazer com seguranca (revert, flag off, rollback, etc) [obrigatorio em MED/HIGH]
- Paths sensiveis: explicitar se tocar auth/, billing/, migrations/, infra/, permissions/

Gatilho "Design Note" (leve, 5-10 linhas):
- Obrigatorio se Risco HIGH OU tocar paths sensiveis OU mudar contrato publico
- Pode ser comentario na issue (decisao, alternativa descartada, impacto, rollout/backout)

Risk Scoring (deterministico) para classificar substantial:
- +3: toca auth/billing/permissions/migrations/infra
- +2: muda API/contrato publico (inclui schemas, endpoints, eventos)
- +2: diff previsto > 200 linhas OU > 3 modulos
- +1: mexe em concorrencia/performance/caches
- +1: validacao insuficiente/sem evidencias claras
- Score >= 3 => Substantial (auto-gates/cloud)

Criterio de saida:
- Plano postado na issue + anexado ao PR (quando existir) + risco/score calculado + backout definido (MED/HIGH).

### 3) CODE
Owner: Code247

- Branch curta: `code247/<issue-id>-slug`
- Commits pequenos, mensagem sempre com issue id
- Validacao local rapida: lint, typecheck, unit smoke
- Trunk-based: branch vive no maximo horas, nao dias

Guardrail:
- Alteracoes em paths sensiveis (incl. `.github/workflows/**`) nao podem ser finalizadas como light.

Criterio de saida:
- Branch compila/roda checks locais + commits atomicos (nao mistura concerns).

### 4) SELF-REVIEW
Owner: Code247

- Agente revisa o proprio diff com checklist: dead code, logs/PII, edge cases, naming, testes faltando
- Corrige o que encontrar ANTES de envolver humano
- Abre PR como Draft com descricao completa: problema, solucao, screenshots/logs, riscos, como testar
- Este passo e o que mais reduz retrabalho - captura "bobeira probabilistica" antes de gastar tempo humano

Criterio de saida:
- PR Draft aberto + template completo + evidencias anexadas (logs/prints/links de run local) + checklist ok.

### 5) REVIEW + MERGE
Owner: Code247 (no-human-middle)

- Classificacao automatica: light (auto-merge) vs substantial (auto-gates/cloud)
- Light: agente faz merge apos checks verdes + PR template completo + nenhum path sensivel
- Merge SEMPRE via PR (merge button/merge queue). Push direto no main = proibido.
- Substantial: PR segue com gates automatizados reforcados (scan, cloud re-evaluation, rollback plan obrigatorio) sem parada humana no meio
- Override manual continua disponivel como excecao operacional, nao como etapa padrao

Regra de ouro:
- Na duvida, bloqueia auto-merge e reavalia com cloud + rollback readiness.

Criterio de saida:
- Light: merge efetuado com checks obrigatorios verdes
- Substantial: gates reforcados aprovados + merge efetuado com checks obrigatorios verdes

### 6) CI GATE
Owner: GitHub Actions

- Branch protection: nao mergeia sem status checks obrigatorios
- Checks minimos: lint + unit + build + typecheck
- Security scan em repos criticos (H1)
- Merge queue quando volume justificar (H1) - CI roda em evento merge_group

Politica anti-flaky:
- Se um check falhar, Code247 pode re-run 1x automaticamente.
- Se falhar novamente, abrir issue "CI Flaky" (ou ticket) e marcar PR como `blocked: ci`.
- Nao "force merge" em cima de flaky.

Politica "Red Main" (P0):
- Se main quebrar, prioridade maxima e restaurar main verde antes de novas features.

Criterio de saida:
- Todos required checks verdes no PR/merge group (quando merge queue ativa).

### 7) DEPLOY + DONE
Owner: Pipeline CI/CD

- Merge no main dispara pipeline de deploy (ou release train)
- "Done" no Linear SO quando merge + deploy concluido (ou evidencia equivalente abaixo)
- Se nao tem deploy integrado ainda: merge + release agendado e rastreavel
- Regra absoluta: "Done" sem gate verde e anti-padrao proibido

Definicao operacional de Done (3 niveis):
- Nivel A (ideal): deploy automatico concluido + healthchecks/observabilidade ok => Done
- Nivel B (transitorio): merge + release train agendado com link rastreavel => Ready for Release (nao Done)
- Nivel C (proibido): Done sem evidencia/links => proibido

Criterio de saida:
- Nivel A: Done setado no Linear + links de deploy/run anexados
- Nivel B: Ready for Release setado no Linear + link de release train anexado

---

## Classificacao: Light vs Substantial

O classificador roda automaticamente em cada PR. Override manual e sempre possivel.

| AUTO-MERGE (Light) | AUTO-GATES/CLOUD (Substantial) |
|---|---|
| Diff < 200 linhas alteradas | Migracoes, schema, contratos |
| Nao toca paths sensiveis | Auth / permissions / billing / infra |
| Checks verdes + scan ok | Diffs grandes ou espalhados |
| PR template completo | Risco medium/high no plano |
| Docs, comments, refactor local, typing, lint | Falta evidencia de validacao ou rollback plan |

Efeito do Risk Scoring:
- Score >= 3 => sempre substantial
- Paths sensiveis => sempre substantial
- `.github/workflows/**` => sempre substantial

Regra de ouro: na duvida, bloqueia auto-merge e exige cloud re-evaluation + rollback readiness.

---

## Template do Plano (Step 2)

Todo plano gerado pelo Code247 deve conter estes campos. O plano e postado como comentario na issue do Linear e incluido na descricao do PR.

Campos obrigatorios do plano:
- Objetivo: O que estamos resolvendo e por que (1-2 frases).
- Acceptance Criteria: 3-5 bullets verificaveis (contrato de pronto).
- Nao-objetivos: 1-2 bullets (scope control).
- Mudancas: Lista de arquivos/modulos/areas que serao tocados.
- Estrategia: Feature flag? Rollout gradual? Migracao? Deploy direto?
- Risco: LOW / MEDIUM / HIGH + justificativa em 1 frase.
- Validacao: Que testes, checks ou observabilidade provam que esta certo.
- Paths sensiveis: Lista explicita se tocar auth/, billing/, migrations/, infra/, permissions/.
- Backout/Rollback: como desfazer (obrigatorio em MED/HIGH).

Auto-classificacao de risco: Code247 marca automaticamente como HIGH quando detecta migracao/db, auth, billing, infra, mudancas em mais de 3 modulos, ou diffs previstos acima de 200 linhas.

Risk Scoring (deterministico): soma de pesos (ver Step 2). Score >= 3 => substantial.

---

## Checklist de Self-Review (Step 4)

O Code247 executa este checklist contra o proprio diff antes de abrir o PR:

- [ ] Dead code removido? Imports nao usados?
- [ ] Logs nao expoe PII ou dados sensiveis?
- [ ] Edge cases cobertos? (null, vazio, limites, concorrencia)
- [ ] Naming consistente com o codebase?
- [ ] Testes adicionados/atualizados? (obrigatorio em bugfix)
- [ ] Nenhuma credencial, secret ou token hardcoded?
- [ ] Nenhuma alteracao em `.github/workflows/**` sem escalacao para cloud re-evaluation?
- [ ] Dependencias alteradas? (lockfile atualizado + scan + testes)
- [ ] PR template preenchido completamente?
- [ ] Diff faz sentido como unidade atomica? (nao mistura concerns)

---

## Gates Obrigatorios (CI)

Checks Minimos (todo repo):
- Lint (eslint, clippy, etc)
- Typecheck (tsc, mypy, etc)
- Unit tests
- Build passa

Branch Protection (obrigatorio):
- Require status checks to pass before merging
- Require pull request reviews (para substantial)
- No direct push to main
- Include administrators (sem excecao)

Politicas operacionais:
- Flaky checks: re-run 1x; reincidencia => ticket CI Flaky + blocked
- Red main: P0, restaurar main verde antes de novas features

Quando adicionar Merge Queue (H1):
- Ativar quando o volume de PRs simultaneos causar "green-on-stale" (check verde em branch desatualizada).
- CI precisa rodar no evento `merge_group`.
- Priorizar nos repos com mais atividade do Code247.

---

## Eventos do Sistema

Mapa dos eventos que o Code247 precisa emitir e escutar para manter o ciclo sincronizado:

| Evento | Origem | Acao |
|---|---|---|
| intention.created | Ecossistema | Code247 recebe, dedup, cria issue no Linear |
| issue.ready | Linear | Code247 faz claim e inicia job |
| issue.updated | Linear | Code247 reavalia prioridade/escopo (idempotente; nao duplica job) |
| plan.completed | Code247 | Plano postado na issue, branch criada |
| pr.opened | GitHub | CI inicia, self-review concluido |
| checks.passed | GitHub Actions | Classifica light/substantial |
| pr.approved | Humano (substantial) | Merge autorizado |
| pr.merged | GitHub | Deploy pipeline inicia |
| deploy.completed | CI/CD | Code247 seta Done no Linear |
| pr.changes_requested | Humano | Code247 consome feedback, itera |
| ci.flaky_detected | CI/CD | Code247 abre ticket + marca PR blocked: ci |
| main.red | CI/CD | Code247 prioriza restaurar main verde (P0) |

---

## Roadmap de Hardening

Priorizacao pragmatica. H0 e bloqueante para operar. H1 e H2 sao incrementais.

| Fase | O que fazer | Status |
|---|---|---|
| H0 | Branch protection + required checks no main (sem excecao) | OBRIGATORIO AGORA |
| H0 | Merge policy light/substantial implementada (classificador + override) | OBRIGATORIO AGORA |
| H0 | Done no Linear so pos-merge + checks (idealmente pos-deploy) | OBRIGATORIO AGORA |
| H0 | Politica anti-flaky + red main (operacao trunk-based real) | OBRIGATORIO AGORA |
| H0 | Guardrails de supply chain (paths sensiveis + workflows + deps) | OBRIGATORIO AGORA |
| H1 | Merge queue para evitar green-on-stale (volume de PRs) | PROXIMO PASSO |
| H1 | Security scan obrigatorio em repos criticos | PROXIMO PASSO |
| H2 | Policy-as-prompt/code: regras auditaveis do que agente pode tocar | HORIZONTE |
| H2 | Feedback loop: persistir motivos de rejeicao como contexto futuro | HORIZONTE |

---

## Feedback Loop (o que falta)

O ciclo acima funciona para execucao, mas para o Code247 melhorar com o tempo, precisa de um loop de aprendizado:

- Quando um PR e rejeitado ou recebe "request changes", persistir o motivo estruturado
- Usar motivos de rejeicao como contexto no proximo plano da mesma area/modulo
- Medir: taxa de first-pass approval, tempo medio de review, rejeicoes por categoria
- Revisar metricas semanalmente para ajustar heuristicas do classificador light/substantial

Isso e o que separa "agente que abre PR" de "agente que melhora com o tempo".

---

## Resumo Executivo

O CICLO EM 1 FRASE:
- Plano curto e explicito + PR como unidade de auditoria + checks obrigatorios + auto-merge so no que e claramente baixo risco + Done so depois de CI/deploy.

---

## Proxima Acao

- Confirmar: GitHub Actions e o runner de CI/deploy? (muda desenho dos gates)
- Implementar H0 completo antes de qualquer outra coisa
- Definir lista de repos criticos para security scan (H1)
- Agendar revisao semanal de metricas do ciclo
- Definir allowlist de comandos do agente e paths sensiveis por repo (Apice do H0/H2)
