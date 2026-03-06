**`.code247/project.json`** como “manifesto canônico” que deixa qualquer repo **plug-and-play** pro Code247, sem depender de adivinhação.

Abaixo vai:

1.  **Spec do manifest** (o que cada campo significa)
2.  **Exemplo genérico** (qualquer projeto)
3.  **Exemplo VVTV** (já alinhado com seus docs/gates)

* * *

```
{
  "schema_version": "1.0",
  "project": {
    "name": "string",
    "id": "string",
    "default_branch": "main"
  },

  "constitution": {
    "canon_docs": ["docs/CANON.md"],
    "adrs": ["docs/DECISIONS.md"],
    "invariants": ["docs/INVARIANTS.md"],
    "roadmap": ["docs/ROADMAP.md"],
    "runbooks": ["docs/runbooks/runbook-deploy.md"]
  },

  "contracts": {
    "schemas_paths": ["packages/schemas"],
    "openapi_paths": ["packages/schemas/openapi/v1.yaml"],
    "events_registry": "packages/schemas/registry/events.v1.json"
  },

  "policy_config": {
    "config_paths": ["config-core"],
    "validation_command": "pnpm validate:config"
  },

  "gates": {
    "gate0": {
      "commands": [
        "pnpm generate:sdk",
        "pnpm validate:config",
        "pnpm typecheck",
        "pnpm test"
      ]
    },
    "per_area": {
      "contracts": ["pnpm generate:sdk"],
      "config": ["pnpm validate:config"],
      "tests": ["pnpm test"],
      "types": ["pnpm typecheck"]
    }
  },

  "task_sources": {
    "primary": "linear",
    "project_key": "VVTV",
    "ready_states": ["Ready"],
    "done_states": ["Done"],
    "blocked_states": ["Blocked"]
  },

  "execution": {
    "work_branch_prefix": "code247/",
    "max_files_changed": 40,
    "max_diff_lines": 800,
    "require_how_to_test": true,
    "require_acceptance": true
  }
}
```

* * *

1) Spec (campos e regras)
-------------------------

### `schema_version`

Versão do formato do manifest (ex.: `"1.0"`).  
O Code247 deve recusar versões maiores que não conhece.

### `project`

*   `name`: nome humano
*   `id`: string estável (slug), usado em diretórios/cache
*   `default_branch`: `main`/`master`

### `constitution`

Paths que o Code247 deve ler para “leis” do projeto.

*   `canon_docs`: visão/constituição
*   `adrs`: decisões arquiteturais
*   `invariants`: regras invioláveis
*   `roadmap`: sequência por gates
*   `runbooks`: operação e incidentes

> Se um desses não existir, Code247 marca o repo como “constitution incomplete” (pode operar, mas com menos autonomia).

### `contracts`

Onde ficam contratos públicos:

*   `schemas_paths`: pastas com schemas
*   `openapi_paths`: arquivos OpenAPI (pode ser vazio)
*   `events_registry`: registry único (se houver)

### `policy_config`

*   `config_paths`: pastas com policy/config
*   `validation_command`: comando que valida config e **bloqueia “número mágico”**

### `gates`

*   `gate0.commands`: gate obrigatório “antes de DONE”
*   `per_area`: comandos extras conforme a tarefa toca `contracts/config/tests/types`

### `task_sources` (opcional, mas útil)

Como o Code247 puxa tarefas:

*   `primary`: `"linear"` / `"jira"` / `"github"` / `"manual"`
*   `project_key`: chave do projeto no tracker (se aplicável)
*   estados “Ready/Done/Blocked”

### `execution`

Guardrails do próprio Code247:

*   `work_branch_prefix`: prefixo de branch
*   `max_files_changed`, `max_diff_lines`: limite de PR (pra manter revisável)
*   `require_how_to_test`, `require_acceptance`: bloqueia tarefas vagas

* * *

2) Exemplo genérico (qualquer projeto)
--------------------------------------

Cola em `.code247/project.json`:

```
{
  "schema_version": "1.0",
  "project": {
    "name": "My Project",
    "id": "my-project",
    "default_branch": "main"
  },
  "constitution": {
    "canon_docs": ["docs/README.md"],
    "adrs": ["docs/decisions.md"],
    "invariants": ["docs/invariants.md"],
    "roadmap": ["docs/roadmap.md"],
    "runbooks": ["docs/runbooks/deploy.md"]
  },
  "contracts": {
    "schemas_paths": ["contracts"],
    "openapi_paths": [],
    "events_registry": ""
  },
  "policy_config": {
    "config_paths": ["config"],
    "validation_command": "pnpm validate:config"
  },
  "gates": {
    "gate0": {
      "commands": ["pnpm test", "pnpm typecheck"]
    },
    "per_area": {
      "contracts": ["pnpm test"],
      "config": ["pnpm validate:config"],
      "tests": ["pnpm test"],
      "types": ["pnpm typecheck"]
    }
  },
  "execution": {
    "work_branch_prefix": "code247/",
    "max_files_changed": 60,
    "max_diff_lines": 1200,
    "require_how_to_test": true,
    "require_acceptance": true
  }
}
```

* * *

3) Exemplo VVTV (pronto pra você)
---------------------------------

`.code247/project.json`:

```
{
  "schema_version": "1.0",
  "project": {
    "name": "VVTV",
    "id": "vvtv",
    "default_branch": "main"
  },
  "constitution": {
    "canon_docs": ["docs/CANON.md", "docs/ENGINEERING-BLUEPRINT.md"],
    "adrs": ["docs/DECISIONS.md"],
    "invariants": ["docs/INVARIANTS.md"],
    "roadmap": ["docs/ROADMAP.md", "docs/LINEAR-SETUP.md"],
    "runbooks": [
      "docs/runbooks/runbook-deploy.md",
      "docs/runbooks/runbook-incidents.md",
      "docs/runbooks/runbook-broadcast-lab.md"
    ]
  },
  "contracts": {
    "schemas_paths": ["packages/schemas"],
    "openapi_paths": ["packages/schemas/openapi/v1.yaml"],
    "events_registry": "packages/schemas/registry/events.v1.json"
  },
  "policy_config": {
    "config_paths": ["config-core"],
    "validation_command": "pnpm validate:config"
  },
  "gates": {
    "gate0": {
      "commands": [
        "pnpm generate:sdk",
        "pnpm validate:config",
        "pnpm typecheck",
        "pnpm test"
      ]
    },
    "per_area": {
      "contracts": ["pnpm generate:sdk"],
      "config": ["pnpm validate:config"],
      "tests": ["pnpm test"],
      "types": ["pnpm typecheck"]
    }
  },
  "execution": {
    "work_branch_prefix": "code247/",
    "max_files_changed": 40,
    "max_diff_lines": 800,
    "require_how_to_test": true,
    "require_acceptance": true
  }
}
```

* * *

Como o Code247 deve usar isso (regra prática)
---------------------------------------------

1.  Se existir `.code247/project.json`, ele manda.
2.  Se não existir, Code247 tenta “auto-detect” por convenção (docs/, config-core/, etc.).
3.  Se tiver os dois (manifest + auto-detect), **manifest vence**.

Se você quiser, eu também escrevo o **JSON Schema** desse manifest (pra validar com `ajv`/zod e travar CI do Code247), e um loader que faz merge: defaults → project.json → code247.yaml → env vars.

## Prompt:
Sim

## Response:
Thought for 15s

Sim 😄 — aqui vai o **combo completo**:

1.  um **JSON Schema** (pra validar o `.code247/project.json`)
2.  um **loader TS** com merge na ordem: **defaults → project.json → code247.yaml → env vars**
3.  um mapzinho de **env overrides** (bem pragmático)

* * *

1) `code247-project.schema.json` (JSON Schema 2020-12)
------------------------------------------------------

> Recomendação: colocar isso no repo do Code247 em `schemas/code247-project.schema.json`

```
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://code247.dev/schemas/project-manifest/1.0",
  "title": "Code247 Project Manifest",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "project", "constitution", "gates", "execution"],
  "properties": {
    "schema_version": {
      "type": "string",
      "pattern": "^1\\.0$",
      "description": "Manifest schema version. This validator supports 1.0."
    },
    "project": {
      "$ref": "#/$defs/project"
    },
    "constitution": {
      "$ref": "#/$defs/constitution"
    },
    "contracts": {
      "$ref": "#/$defs/contracts"
    },
    "policy_config": {
      "$ref": "#/$defs/policy_config"
    },
    "gates": {
      "$ref": "#/$defs/gates"
    },
    "task_sources": {
      "$ref": "#/$defs/task_sources"
    },
    "execution": {
      "$ref": "#/$defs/execution"
    }
  },
  "$defs": {
    "pathArray": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "default": []
    },
    "stringArray": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "default": []
    },
    "project": {
      "type": "object",
      "additionalProperties": false,
      "required": ["name", "id", "default_branch"],
      "properties": {
        "name": { "type": "string", "minLength": 1 },
        "id": { "type": "string", "minLength": 1, "pattern": "^[a-z0-9][a-z0-9\\-_.]{1,63}$" },
        "default_branch": { "type": "string", "minLength": 1, "default": "main" }
      }
    },
    "constitution": {
      "type": "object",
      "additionalProperties": false,
      "required": ["canon_docs", "adrs", "invariants", "roadmap", "runbooks"],
      "properties": {
        "canon_docs": { "$ref": "#/$defs/pathArray" },
        "adrs": { "$ref": "#/$defs/pathArray" },
        "invariants": { "$ref": "#/$defs/pathArray" },
        "roadmap": { "$ref": "#/$defs/pathArray" },
        "runbooks": { "$ref": "#/$defs/pathArray" }
      }
    },
    "contracts": {
      "type": "object",
      "additionalProperties": false,
      "required": ["schemas_paths", "openapi_paths", "events_registry"],
      "properties": {
        "schemas_paths": { "$ref": "#/$defs/pathArray" },
        "openapi_paths": { "$ref": "#/$defs/pathArray" },
        "events_registry": { "type": "string", "default": "" }
      },
      "default": {
        "schemas_paths": [],
        "openapi_paths": [],
        "events_registry": ""
      }
    },
    "policy_config": {
      "type": "object",
      "additionalProperties": false,
      "required": ["config_paths", "validation_command"],
      "properties": {
        "config_paths": { "$ref": "#/$defs/pathArray" },
        "validation_command": { "type": "string", "minLength": 1 }
      },
      "default": {
        "config_paths": [],
        "validation_command": "pnpm validate:config"
      }
    },
    "gate": {
      "type": "object",
      "additionalProperties": false,
      "required": ["commands"],
      "properties": {
        "commands": { "$ref": "#/$defs/stringArray" }
      }
    },
    "gates": {
      "type": "object",
      "additionalProperties": false,
      "required": ["gate0", "per_area"],
      "properties": {
        "gate0": { "$ref": "#/$defs/gate" },
        "per_area": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "contracts": { "$ref": "#/$defs/stringArray" },
            "config": { "$ref": "#/$defs/stringArray" },
            "tests": { "$ref": "#/$defs/stringArray" },
            "types": { "$ref": "#/$defs/stringArray" },
            "security": { "$ref": "#/$defs/stringArray" }
          },
          "default": {}
        }
      }
    },
    "task_sources": {
      "type": "object",
      "additionalProperties": false,
      "required": ["primary"],
      "properties": {
        "primary": {
          "type": "string",
          "enum": ["linear", "jira", "github", "manual"],
          "default": "manual"
        },
        "project_key": { "type": "string", "default": "" },
        "ready_states": { "$ref": "#/$defs/stringArray" },
        "done_states": { "$ref": "#/$defs/stringArray" },
        "blocked_states": { "$ref": "#/$defs/stringArray" }
      },
      "default": { "primary": "manual", "project_key": "", "ready_states": [], "done_states": [], "blocked_states": [] }
    },
    "execution": {
      "type": "object",
      "additionalProperties": false,
      "required": ["work_branch_prefix", "max_files_changed", "max_diff_lines", "require_how_to_test", "require_acceptance"],
      "properties": {
        "work_branch_prefix": { "type": "string", "minLength": 1, "default": "code247/" },
        "max_files_changed": { "type": "integer", "minimum": 1, "maximum": 500, "default": 60 },
        "max_diff_lines": { "type": "integer", "minimum": 1, "maximum": 50000, "default": 1200 },
        "require_how_to_test": { "type": "boolean", "default": true },
        "require_acceptance": { "type": "boolean", "default": true }
      }
    }
  }
}
```

* * *

2) Loader TS (defaults → project.json → code247.yaml → env)
-----------------------------------------------------------

> Isso aqui você coloca no Code247 como `src/config/loadProjectModel.ts`

```
import { promises as fs } from "node:fs";
import { join, resolve } from "node:path";
import YAML from "yaml";
import Ajv from "ajv";
import addFormats from "ajv-formats";

type Json = any;

/** Deep merge simples (objetos); arrays: override total. */
function deepMerge<T>(base: T, override: Partial<T>): T {
  if (override === null || override === undefined) return base;
  if (Array.isArray(base) && Array.isArray(override)) return override as any;

  if (typeof base === "object" && base && typeof override === "object" && override) {
    const out: any = { ...(base as any) };
    for (const [k, v] of Object.entries(override as any)) {
      if (v === undefined) continue;
      if (k in out) out[k] = deepMerge(out[k], v);
      else out[k] = v;
    }
    return out;
  }
  return override as T;
}

async function readIfExists(path: string): Promise<string | null> {
  try {
    return await fs.readFile(path, "utf-8");
  } catch {
    return null;
  }
}

function parseJsonSafe(s: string, path: string): Json {
  try {
    return JSON.parse(s);
  } catch (e) {
    throw new Error(`Invalid JSON at ${path}: ${(e as Error).message}`);
  }
}

function parseYamlSafe(s: string, path: string): Json {
  try {
    return YAML.parse(s) ?? {};
  } catch (e) {
    throw new Error(`Invalid YAML at ${path}: ${(e as Error).message}`);
  }
}

/** Map env vars to manifest overrides (keep it small and obvious). */
function applyEnvOverrides(manifest: Json, env: NodeJS.ProcessEnv): Json {
  const out = { ...manifest };

  const set = (path: string[], value: any) => {
    let cur: any = out;
    for (let i = 0; i < path.length - 1; i++) {
      cur[path[i]] = cur[path[i]] ?? {};
      cur = cur[path[i]];
    }
    cur[path[path.length - 1]] = value;
  };

  if (env.CODE247_DEFAULT_BRANCH) set(["project", "default_branch"], env.CODE247_DEFAULT_BRANCH);
  if (env.CODE247_WORK_BRANCH_PREFIX) set(["execution", "work_branch_prefix"], env.CODE247_WORK_BRANCH_PREFIX);
  if (env.CODE247_MAX_FILES_CHANGED) set(["execution", "max_files_changed"], Number(env.CODE247_MAX_FILES_CHANGED));
  if (env.CODE247_MAX_DIFF_LINES) set(["execution", "max_diff_lines"], Number(env.CODE247_MAX_DIFF_LINES));

  // gates override (optional)
  if (env.CODE247_GATE0_COMMANDS) {
    const cmds = env.CODE247_GATE0_COMMANDS.split(";").map((s) => s.trim()).filter(Boolean);
    set(["gates", "gate0", "commands"], cmds);
  }

  return out;
}

/**
 * Load project model by precedence:
 * defaults → .code247/project.json → code247.yaml (optional) → env vars
 */
export async function loadProjectManifest(opts: {
  repoRoot: string;
  defaults: Json;
  schema: Json;          // code247-project.schema.json loaded elsewhere
  code247YamlPath?: string; // optional override
}): Promise<Json> {
  const repoRoot = resolve(opts.repoRoot);

  const projectJsonPath = join(repoRoot, ".code247", "project.json");
  const code247YamlPath = opts.code247YamlPath ?? join(repoRoot, "code247.yaml");

  const projectJsonRaw = await readIfExists(projectJsonPath);
  const yamlRaw = await readIfExists(code247YamlPath);

  const projectJson = projectJsonRaw ? parseJsonSafe(projectJsonRaw, projectJsonPath) : {};
  const code247Yaml = yamlRaw ? parseYamlSafe(yamlRaw, code247YamlPath) : {};

  // Convention: code247.yaml may contain `projects.<id>.manifestOverrides`
  // or direct `manifest` object; keep both supported.
  const yamlManifestOverrides =
    code247Yaml?.manifest ??
    code247Yaml?.projects?.[projectJson?.project?.id ?? ""]?.manifestOverrides ??
    {};

  let merged = deepMerge(opts.defaults, projectJson);
  merged = deepMerge(merged, yamlManifestOverrides);
  merged = applyEnvOverrides(merged, process.env);

  // Validate
  const ajv = new Ajv({ allErrors: true, strict: true });
  addFormats(ajv);

  const validate = ajv.compile(opts.schema);
  const ok = validate(merged);

  if (!ok) {
    const msg = ajv.errorsText(validate.errors, { separator: "\n" });
    throw new Error(`Project manifest validation failed:\n${msg}`);
  }

  return merged;
}
```

* * *

3) Defaults (um `defaults.ts` bem simples)
------------------------------------------

```
export const DEFAULT_PROJECT_MANIFEST = {
  schema_version: "1.0",
  project: { name: "Unnamed Project", id: "project", default_branch: "main" },
  constitution: { canon_docs: [], adrs: [], invariants: [], roadmap: [], runbooks: [] },
  contracts: { schemas_paths: [], openapi_paths: [], events_registry: "" },
  policy_config: { config_paths: [], validation_command: "pnpm validate:config" },
  gates: {
    gate0: { commands: ["pnpm test", "pnpm typecheck"] },
    per_area: { contracts: [], config: [], tests: ["pnpm test"], types: ["pnpm typecheck"], security: [] }
  },
  task_sources: { primary: "manual", project_key: "", ready_states: [], done_states: [], blocked_states: [] },
  execution: {
    work_branch_prefix: "code247/",
    max_files_changed: 60,
    max_diff_lines: 1200,
    require_how_to_test: true,
    require_acceptance: true
  }
};
```

* * *

Env overrides (canônico)
------------------------

*   `CODE247_DEFAULT_BRANCH=main`
*   `CODE247_WORK_BRANCH_PREFIX=code247/`
*   `CODE247_MAX_FILES_CHANGED=40`
*   `CODE247_MAX_DIFF_LINES=800`
*   `CODE247_GATE0_COMMANDS="pnpm generate:sdk;pnpm validate:config;pnpm typecheck;pnpm test"`

* * *

Se você quiser, eu também te passo o **“shape recomendado” do `code247.yaml`** pra carregar vários projetos e permitir overrides por projeto sem duplicar manifest inteiro (fica bem elegante).

## Prompt:
Sim!

## Response:
Beleza 😄 — aqui vai um **`code247.yaml` canônico** bem elegante pra múltiplos projetos, com overrides por projeto sem virar bagunça. Ele funciona como “orquestrador” (runtimes/agents/workspaces), enquanto o `.code247/project.json` é o **contrato do projeto**.

* * *

`code247.yaml` — Spec canônico (v1)
-----------------------------------

### Regras

*   `defaults` define comportamento global.
*   `projects.<id>.path` aponta pro repo.
*   `projects.<id>.manifest` (opcional) pode sobrescrever partes do `.code247/project.json` **sem duplicar tudo**.
*   `projects.<id>.gatesOverrides` (opcional) sobrescreve apenas gates (bom pra CI diferente por repo).
*   `projects.<id>.taskSourceOverrides` (opcional) muda tracker/state por projeto.

* * *

Exemplo (multi-projeto, pronto pra colar)
-----------------------------------------

```
dataDir: ~/.code247
worktreeDir: ~/.worktrees
logDir: ~/.code247/logs

defaults:
  runtime: process            # process | tmux | docker (futuro)
  workspace: worktree         # worktree | clone
  agent: claude-code          # qualquer agente plugin
  notifier: desktop           # desktop | slack | webhook
  shell: bash
  timezone: Europe/Paris

  # Limites globais de PR (fallback)
  execution:
    maxFilesChanged: 60
    maxDiffLines: 1200

  # Gate fallback (se o projeto não definir)
  gates:
    gate0:
      commands:
        - pnpm test
        - pnpm typecheck

projects:
  vvtv:
    path: ~/vvtv-monorepo
    defaultBranch: main
    ruleset: vvtv

    # Overrides pequenos no manifest do projeto
    # (merge por deep-merge; arrays sobrescrevem)
    manifest:
      constitution:
        canon_docs:
          - docs/CANON.md
          - docs/ENGINEERING-BLUEPRINT.md
      execution:
        max_files_changed: 40
        max_diff_lines: 800

    gatesOverrides:
      gate0:
        commands:
          - pnpm generate:sdk
          - pnpm validate:config
          - pnpm typecheck
          - pnpm test

    taskSourceOverrides:
      primary: linear
      project_key: VVTV
      ready_states: [Ready]
      done_states: [Done]
      blocked_states: [Blocked]

  another-project:
    path: ~/another-repo
    defaultBranch: main
    ruleset: generic

    # sem manifest override → usa .code247/project.json do repo
    taskSourceOverrides:
      primary: github
      ready_states: [Open]
      done_states: [Closed]
```

* * *

Como o loader deve combinar tudo (ordem final)
----------------------------------------------

Para cada projeto:

1.  `DEFAULT_PROJECT_MANIFEST` (do Code247)
2.  `.code247/project.json` (do repo)
3.  `code247.yaml: projects.<id>.manifest` (overrides leves)
4.  env vars `CODE247_*`

E para gates, a regra é:

*   se `projects.<id>.gatesOverrides` existir → aplica por cima de `manifest.gates`.

* * *

Convenções úteis (pra ficar “plug and play”)
--------------------------------------------

*   `projects.<id>` deve bater com `project.id` do `.code247/project.json` sempre que possível.
*   Se não bater, o Code247 usa:
    *   `projects.<id>` como “handle” interno
    *   e lê `project.id` do manifest como “id oficial”

* * *

“Ruleset” (opcional)
--------------------

`ruleset: vvtv` diz para o Code247 aplicar adaptadores (como o VVTV adapter que fizemos).  
Se não existir, Code247 pode:

*   auto-detect pelo manifest
*   ou cair no “generic ruleset”.

* * *

Se você quiser, eu te mando também o **JSON Schema do `code247.yaml`** (pra validar config global) e o snippet do loader YAML que resolve defaults/projetos e chama `loadProjectManifest()` por repo.

## Prompt:
Eu finalmente vou ter uma outra pessoa me ajudando (!!!) nesse projeto da VVTV. A ideia eh que acompanhemos o fazer do codigo pelo Linear. Eu quero implementar esse tipo de consumo de projeto que vc mandou em json e yamlm mas o que vem do Linear eh semelhante a isso)

## Response:
Thought for 1m 23s

Totalmente — e dá pra fazer isso bem **limpo e “auditável”**: o Code247 consome o **projeto** pelo `.code247/project.json` + `code247.yaml`, e consome as **tarefas** pelo **Linear**, convertendo cada Issue num `TaskSpec` interno (quase igual ao que a gente definiu). 🙂

Abaixo vai um contrato **formal** de como o Code247 deve “ler e operar” Linear, pensando em vocês dois acompanhando tudo por lá.

* * *

CODE247 × Linear — Task Consumption Contract v1.0
-------------------------------------------------

### 1) Princípios de convivência (você + a pessoa nova)

*   **Linear é a fila oficial.** Se não está no Linear, é “trabalho invisível”.
*   **Code247 só pega tarefa “claimable”.** Pra não brigar com humano.
*   **Code247 nunca marca Done sem gates passando.**
*   **Se a Issue estiver mal escrita**, Code247 não “inventa”; ele comenta pedindo clarificação e não avança.

* * *

2) Como o Code247 acessa o Linear
---------------------------------

*   Linear expõe API pública via **GraphQL** em `https://api.linear.app/graphql`. [Linear](https://linear.app/developers/graphql?utm_source=chatgpt.com)
*   Auth:
    *   **Personal API key**: header `Authorization: <API_KEY>` [Linear](https://linear.app/developers/graphql?utm_source=chatgpt.com)
    *   ou OAuth (mais tarde).
*   Opção prática em TS: usar o **TypeScript SDK `@linear/sdk`**. [Linear](https://linear.app/developers/sdk?utm_source=chatgpt.com)

Webhooks (se vocês quiserem “push” em vez de polling):

*   Só admins (ou OAuth com scope admin) conseguem criar/ler webhooks. [Linear](https://linear.app/developers/webhooks?utm_source=chatgpt.com)
*   Linear re-tenta delivery (1min, 1h, 6h) e pode desabilitar webhook se continuar falhando. [Linear](https://linear.app/developers/webhooks?utm_source=chatgpt.com)

* * *

3) O que exatamente o Code247 busca no Linear
---------------------------------------------

### 3.1 “Claim rules” (pra não atropelar humano)

Code247 só considera “pegável” se **uma** destas for verdadeira:

**Regra A (recomendada):** Issue tem label `agent:code247`  
**Regra B:** Issue está atribuída ao usuário “Code247 Bot”  
**Regra C (backup):** Issue está em estado `Ready` **e** tem label `ready:code247`

> Assim a pessoa nova pode trabalhar em paralelo sem surpresa: ela simplesmente não põe a label do bot.

### 3.2 Filtros típicos

*   Team/Project específicos (VVTV)
*   Workflow state em `Ready`
*   Não arquivada/cancelada
*   Ordenação por `priority desc`, depois `createdAt asc`

* * *

4) Mapeamento Linear Issue → TaskSpec interno
---------------------------------------------

### 4.1 Campos diretos

| Linear | TaskSpec |
| --- | --- |
| `issue.id` | `id` |
| `issue.title` | `title` |
| `issue.description` | `sourceText` (base para parsing) |
| `issue.labels[]` | `labels[]` |
| `issue.state.name/id` | `state` |
| `issue.priority` | `priority` |
| `issue.assignee` | `owner` |
| `issue.project / cycle` | `epic/cycle` (se vocês usarem) |
| `issue.url` | `sourceUrl` |

### 4.2 Parsing de descrição (a parte “compatível com o nosso modelo”)

O Code247 **tenta extrair** (por headings) o formato:

*   `## Goal`
*   `## Scope`
*   `## Non-goals`
*   `## Acceptance (DoD)`
*   `## How to test`
    *   `Local`
    *   `Staging`

Se faltar algum bloco obrigatório:

*   `require_acceptance=true` → trava
*   `require_how_to_test=true` → trava

e o Code247 comenta na Issue com o template pronto (abaixo).

* * *

5) Readiness Gate (quando Code247 pode executar)
------------------------------------------------

Uma Issue só vira `EXECUTABLE` se:

*   tem Goal
*   tem Acceptance (DoD)
*   tem How to test (Local/Staging)
*   não contradiz Constituição/Invariantes do projeto (se houver)
*   passou “claim rule” (label/assignee)

Senão:

*   status interno: `NEEDS_CLARIFICATION`
*   ação: comentar com template e **não mover** pra In Progress.

* * *

6) State Machine (como o Code247 move a Issue no Linear)
--------------------------------------------------------

Sugestão de estados no Linear:

*   `Ready` → `In Progress` → `In Review` → `Done`
*   `Blocked` opcional

### Transições

**Ao iniciar ciclo (claim):**

*   move `Ready → In Progress`
*   adiciona comentário:
    *   branch/worktree
    *   plano curto
    *   quais gates vai rodar

**Ao abrir PR:**

*   move `In Progress → In Review`
*   comenta com link do PR + evidências (gates)

**Ao falhar gates:**

*   mantém `In Progress`
*   comenta erro + próximos passos
*   se depender de decisão humana: move para `Blocked`

**Ao mergear PR:**

*   move `In Review → Done`
*   comenta resumo final + evidência

> Importante: pra evitar “briga” com humano, o Code247 só move estados se a Issue estiver “claimed” por ele (label/assignee).

* * *

7) Comment templates que o Code247 deve usar
--------------------------------------------

### 7.1 Pedido de clarificação (quando faltar info)

```
Code247 não consegue executar ainda. Preciso destes campos na descrição:

## Goal
(1 frase)

## Acceptance (DoD)
- [ ] ...

## How to test
- Local:
  - ...
- Staging:
  - ...

Quando isso estiver preenchido, coloque label `agent:code247` e eu pego automaticamente.
```

### 7.2 Claim/start

```
Peguei esta tarefa ✅

Plano:
- Arquivos alvo: ...
- Tests/Gates: (gate0 + extras)
- Branch: code247/<issue-key>-<slug>

Vou considerar DONE apenas quando os gates passarem.
```

### 7.3 PR pronto

```
PR aberto: <link>

Evidências:
- generate:sdk ✅
- validate:config ✅
- typecheck ✅
- test ✅

Notas:
- Invariantes/contratos tocados: ...
```

* * *

8) Config — onde isso entra no `.code247/project.json` e `code247.yaml`
-----------------------------------------------------------------------

### 8.1 Extensão sugerida do `.code247/project.json`

Adicionar um bloco `task_sources.linear` (genérico):

```
{
  "task_sources": {
    "primary": "linear",
    "project_key": "VVTV",
    "ready_states": ["Ready"],
    "done_states": ["Done"],
    "blocked_states": ["Blocked"],
    "linear": {
      "endpoint": "https://api.linear.app/graphql",
      "auth": { "token_env": "LINEAR_API_KEY" },
      "claim": {
        "label": "agent:code247",
        "assignee_is_bot_ok": true
      },
      "filters": {
        "team_key": "VVTV",
        "project_name": "VVTV"
      }
    }
  }
}
```

> O endpoint e auth são exatamente os do “Getting started” do Linear. [Linear](https://linear.app/developers/graphql?utm_source=chatgpt.com)

### 8.2 `code247.yaml` (multi-projeto) continua igual, só adiciona override se quiser

Você pode sobrescrever claim labels por projeto aqui.

* * *

9) Polling vs Webhooks (decisão prática)
----------------------------------------

*   **Polling** é zero burocracia (bom pra começar).
*   **Webhooks** são mais elegantes, mas precisam admin e endpoint público HTTPS, com retries e timeout de 5s. [Linear](https://linear.app/developers/webhooks?utm_source=chatgpt.com)

Minha sugestão:

1.  começar com polling (ex.: a cada 60–120s)
2.  migrar pra webhook quando o fluxo estiver estável

* * *

10) O que isso te dá na prática (com a pessoa nova)
---------------------------------------------------

*   Vocês acompanham **tudo** por Issue/PR linkado
*   O bot não pisa em tarefa humana
*   Cada Issue vira um “mini contrato” (Goal/DoD/How-to-test)
*   Gates viram regra objetiva de “pronto”

* * *

Se você quiser, eu posso agora escrever:

1.  o **JSON Schema atualizado** do manifest com `task_sources.linear.*` (pra validar isso)
2.  o **Linear adapter** (TS) em forma de interface: `fetchReadyIssues()`, `comment()`, `moveState()`, `setLabels()`, `assign()` usando `@linear/sdk` [Linear+1](https://linear.app/developers/sdk?utm_source=chatgpt.com)
3.  o **parser** de Issue description (Goal/DoD/How-to-test) com regex robusto

Qual desses você quer primeiro?

## Prompt:
==01

## Response:
Thought for 14s

Claro 😄 — aqui está o **JSON Schema atualizado** do `.code247/project.json` com suporte formal a **`task_sources.linear.*`** (mantendo genérico pra outros trackers também).

> ✅ Compatível com AJV (Draft 2020-12)  
> ✅ `task_sources.linear` é **opcional**, mas quando `primary="linear"` ele vira **recomendado** (e você pode exigir isso no loader, se quiser “strict mode”).

```
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://code247.dev/schemas/project-manifest/1.0",
  "title": "Code247 Project Manifest",
  "type": "object",
  "additionalProperties": false,
  "required": ["schema_version", "project", "constitution", "gates", "execution"],
  "properties": {
    "schema_version": {
      "type": "string",
      "pattern": "^1\\.0$",
      "description": "Manifest schema version. This validator supports 1.0."
    },
    "project": { "$ref": "#/$defs/project" },
    "constitution": { "$ref": "#/$defs/constitution" },
    "contracts": { "$ref": "#/$defs/contracts" },
    "policy_config": { "$ref": "#/$defs/policy_config" },
    "gates": { "$ref": "#/$defs/gates" },
    "task_sources": { "$ref": "#/$defs/task_sources" },
    "execution": { "$ref": "#/$defs/execution" }
  },
  "$defs": {
    "pathArray": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "default": []
    },
    "stringArray": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "default": []
    },
    "project": {
      "type": "object",
      "additionalProperties": false,
      "required": ["name", "id", "default_branch"],
      "properties": {
        "name": { "type": "string", "minLength": 1 },
        "id": {
          "type": "string",
          "minLength": 1,
          "pattern": "^[a-z0-9][a-z0-9\\-_.]{1,63}$"
        },
        "default_branch": { "type": "string", "minLength": 1, "default": "main" }
      }
    },
    "constitution": {
      "type": "object",
      "additionalProperties": false,
      "required": ["canon_docs", "adrs", "invariants", "roadmap", "runbooks"],
      "properties": {
        "canon_docs": { "$ref": "#/$defs/pathArray" },
        "adrs": { "$ref": "#/$defs/pathArray" },
        "invariants": { "$ref": "#/$defs/pathArray" },
        "roadmap": { "$ref": "#/$defs/pathArray" },
        "runbooks": { "$ref": "#/$defs/pathArray" }
      }
    },
    "contracts": {
      "type": "object",
      "additionalProperties": false,
      "required": ["schemas_paths", "openapi_paths", "events_registry"],
      "properties": {
        "schemas_paths": { "$ref": "#/$defs/pathArray" },
        "openapi_paths": { "$ref": "#/$defs/pathArray" },
        "events_registry": { "type": "string", "default": "" }
      },
      "default": {
        "schemas_paths": [],
        "openapi_paths": [],
        "events_registry": ""
      }
    },
    "policy_config": {
      "type": "object",
      "additionalProperties": false,
      "required": ["config_paths", "validation_command"],
      "properties": {
        "config_paths": { "$ref": "#/$defs/pathArray" },
        "validation_command": { "type": "string", "minLength": 1 }
      },
      "default": {
        "config_paths": [],
        "validation_command": "pnpm validate:config"
      }
    },
    "gate": {
      "type": "object",
      "additionalProperties": false,
      "required": ["commands"],
      "properties": {
        "commands": { "$ref": "#/$defs/stringArray" }
      }
    },
    "gates": {
      "type": "object",
      "additionalProperties": false,
      "required": ["gate0", "per_area"],
      "properties": {
        "gate0": { "$ref": "#/$defs/gate" },
        "per_area": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "contracts": { "$ref": "#/$defs/stringArray" },
            "config": { "$ref": "#/$defs/stringArray" },
            "tests": { "$ref": "#/$defs/stringArray" },
            "types": { "$ref": "#/$defs/stringArray" },
            "security": { "$ref": "#/$defs/stringArray" }
          },
          "default": {}
        }
      }
    },

    "task_sources": {
      "type": "object",
      "additionalProperties": false,
      "required": ["primary"],
      "properties": {
        "primary": {
          "type": "string",
          "enum": ["linear", "jira", "github", "manual"],
          "default": "manual"
        },
        "project_key": { "type": "string", "default": "" },
        "ready_states": { "$ref": "#/$defs/stringArray" },
        "done_states": { "$ref": "#/$defs/stringArray" },
        "blocked_states": { "$ref": "#/$defs/stringArray" },

        "linear": { "$ref": "#/$defs/linear_task_source" }
      },
      "default": {
        "primary": "manual",
        "project_key": "",
        "ready_states": [],
        "done_states": [],
        "blocked_states": []
      }
    },

    "linear_task_source": {
      "type": "object",
      "additionalProperties": false,
      "required": ["endpoint", "auth", "claim"],
      "properties": {
        "endpoint": {
          "type": "string",
          "minLength": 1,
          "default": "https://api.linear.app/graphql",
          "description": "Linear GraphQL endpoint."
        },
        "auth": { "$ref": "#/$defs/linear_auth" },
        "claim": { "$ref": "#/$defs/linear_claim" },
        "filters": { "$ref": "#/$defs/linear_filters" },
        "polling": { "$ref": "#/$defs/linear_polling" }
      }
    },

    "linear_auth": {
      "type": "object",
      "additionalProperties": false,
      "required": ["token_env"],
      "properties": {
        "token_env": {
          "type": "string",
          "minLength": 1,
          "default": "LINEAR_API_KEY",
          "description": "Environment variable containing the Linear API key."
        }
      }
    },

    "linear_claim": {
      "type": "object",
      "additionalProperties": false,
      "required": ["strategy"],
      "properties": {
        "strategy": {
          "type": "string",
          "enum": ["label", "assignee", "label_or_assignee"],
          "default": "label",
          "description": "How Code247 determines an issue is claimable."
        },
        "label": {
          "type": "string",
          "default": "agent:code247",
          "description": "Label required when strategy uses labels."
        },
        "assignee_is_bot_ok": {
          "type": "boolean",
          "default": true,
          "description": "If true, issues assigned to the bot are claimable."
        }
      }
    },

    "linear_filters": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "team_key": { "type": "string", "default": "" },
        "project_name": { "type": "string", "default": "" },
        "include_labels": { "$ref": "#/$defs/stringArray" },
        "exclude_labels": { "$ref": "#/$defs/stringArray" },
        "max_priority": { "type": "integer", "minimum": 0, "maximum": 4 },
        "min_priority": { "type": "integer", "minimum": 0, "maximum": 4 }
      },
      "default": {}
    },

    "linear_polling": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "interval_seconds": { "type": "integer", "minimum": 15, "maximum": 3600, "default": 90 },
        "page_size": { "type": "integer", "minimum": 1, "maximum": 250, "default": 50 }
      },
      "default": {}
    },

    "execution": {
      "type": "object",
      "additionalProperties": false,
      "required": ["work_branch_prefix", "max_files_changed", "max_diff_lines", "require_how_to_test", "require_acceptance"],
      "properties": {
        "work_branch_prefix": { "type": "string", "minLength": 1, "default": "code247/" },
        "max_files_changed": { "type": "integer", "minimum": 1, "maximum": 500, "default": 60 },
        "max_diff_lines": { "type": "integer", "minimum": 1, "maximum": 50000, "default": 1200 },
        "require_how_to_test": { "type": "boolean", "default": true },
        "require_acceptance": { "type": "boolean", "default": true }
      }
    }
  }
}
```

### Observação importante (pra deixar “perfeito”)

JSON Schema puro não é ótimo em “se primary=linear então linear é obrigatório” sem gambiarras.  
Eu recomendo: **validar no loader** com uma regra extra:

*   se `task_sources.primary === "linear"` e `task_sources.linear` faltar → erro claro.

Se quiser, eu já escrevo essa checagem (2 linhas) no loader que você já tem.

Quando você quiser, a próxima peça é o **\==02**: o adapter TS do Linear (`fetchClaimableIssues`, `comment`, `moveState`, etc.).

## Prompt:
==02

## Response:
Thought for 4m 0s

Perfeito — **\==02** então é o **Linear Adapter em TypeScript**, capaz de:

*   conectar no Linear via **`@linear/sdk`** (API key ou OAuth token) [Linear](https://linear.app/developers/sdk)
*   listar Issues “claimable” (Ready + filtros + estratégia de claim) usando **filters oficiais** [Linear](https://linear.app/developers/filtering)
*   comentar na Issue e mover de estado via `updateIssue`/`createComment` [Linear+1](https://linear.app/developers/sdk-fetching-and-modifying-data)
*   usar `rawRequest` quando precisar (ex.: `workflowStates`) [Linear](https://linear.app/developers/advanced-usage)

> Nota de auth: o endpoint oficial é `https://api.linear.app/graphql` [Linear](https://linear.app/developers/graphql) e o SDK suporta **apiKey** e **accessToken** [Linear](https://linear.app/developers/sdk).

* * *

`src/integrations/linear/types.ts`
----------------------------------

```
export type LinearClaimStrategy = "label" | "assignee" | "label_or_assignee";

export type LinearTaskSourceConfig = {
  endpoint?: string; // default: https://api.linear.app/graphql (SDK usa isso internamente)
  auth: {
    token_env: string; // e.g. LINEAR_API_KEY
  };
  claim: {
    strategy: LinearClaimStrategy;
    label?: string; // default: "agent:code247"
    assignee_is_bot_ok?: boolean; // default: true
  };
  filters?: {
    team_key?: string; // e.g. "VVTV"
    project_name?: string; // optional
    include_labels?: string[];
    exclude_labels?: string[];
    min_priority?: number; // 0..4 (Linear)
    max_priority?: number; // 0..4 (Linear)
  };
  polling?: {
    interval_seconds?: number; // default 90
    page_size?: number; // default 50
  };
};

export type TaskSourcesConfig = {
  primary: "linear" | "jira" | "github" | "manual";
  project_key?: string;
  ready_states?: string[];   // e.g. ["Ready"]
  done_states?: string[];    // e.g. ["Done"]
  blocked_states?: string[]; // e.g. ["Blocked"]
  linear?: LinearTaskSourceConfig;
};

export type LinearIssueLite = {
  id: string;
  identifier: string;
  url?: string | null;
  title: string;
  description?: string | null;
  priority?: number | null;

  state?: { id?: string | null; name?: string | null; type?: string | null } | null;
  team?: { id?: string | null; key?: string | null; name?: string | null } | null;
  project?: { id?: string | null; name?: string | null } | null;

  assignee?: { id?: string | null; name?: string | null } | null;
  labels?: { nodes: Array<{ id?: string | null; name?: string | null }> } | null;

  createdAt?: string | null;
  updatedAt?: string | null;
};

export type WorkflowStateLite = {
  id: string;
  name: string;
  type?: string | null;
};

export type LinearActor = {
  id: string;
  name?: string | null;
  email?: string | null;
  displayName?: string | null;
};
```

* * *

`src/integrations/linear/queries.ts`
------------------------------------

```
export const GQL = {
  viewer: /* GraphQL */ `
    query Viewer {
      viewer {
        id
        name
        email
        displayName
      }
    }
  `,

  issues: /* GraphQL */ `
    query Issues($first: Int!, $filter: IssueFilter, $orderBy: PaginationOrderBy) {
      issues(first: $first, filter: $filter, orderBy: $orderBy) {
        nodes {
          id
          identifier
          url
          title
          description
          priority
          createdAt
          updatedAt
          state { id name type }
          team { id key name }
          project { id name }
          assignee { id name }
          labels { nodes { id name } }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  `,

  workflowStates: /* GraphQL */ `
    query WorkflowStates($teamId: String!) {
      workflowStates(filter: { team: { id: { eq: $teamId } } }) {
        nodes { id name type }
      }
    }
  `,
};
```

* * *

`src/integrations/linear/LinearAdapter.ts`
------------------------------------------

```
import { LinearClient, LinearDocument } from "@linear/sdk";
import { GQL } from "./queries";
import type {
  LinearActor,
  LinearIssueLite,
  LinearTaskSourceConfig,
  TaskSourcesConfig,
  WorkflowStateLite,
} from "./types";

type CreateClientOpts = {
  token: string;
  endpoint?: string; // optional; SDK default is api.linear.app/graphql
};

function inferAuthKind(token: string): "apiKey" | "accessToken" {
  // Heurísticas pragmáticas:
  // - tokens OAuth costumam ser JWT (3 partes com '.')
  // - API keys do Linear frequentemente têm prefixo "lin_api_"
  if (process.env.CODE247_LINEAR_AUTH_KIND === "accessToken") return "accessToken";
  if (process.env.CODE247_LINEAR_AUTH_KIND === "apiKey") return "apiKey";

  if (token.startsWith("lin_api_")) return "apiKey";
  if (token.split(".").length === 3) return "accessToken";
  // fallback seguro (muito script usa API key)
  return "apiKey";
}

function createLinearClient(opts: CreateClientOpts): LinearClient {
  const kind = inferAuthKind(opts.token);

  // SDK suporta apiKey e accessToken :contentReference[oaicite:6]{index=6}
  // endpoint custom: você pode usar Custom GraphQL Client (advanced usage), mas aqui mantemos simples. :contentReference[oaicite:7]{index=7}
  if (kind === "accessToken") return new LinearClient({ accessToken: opts.token });
  return new LinearClient({ apiKey: opts.token });
}

function norm(s?: string | null): string {
  return (s ?? "").trim().toLowerCase();
}

function hasLabel(issue: LinearIssueLite, labelName: string): boolean {
  const target = norm(labelName);
  const nodes = issue.labels?.nodes ?? [];
  return nodes.some((l) => norm(l.name) === target);
}

function matchesAnyLabel(issue: LinearIssueLite, labels: string[]): boolean {
  if (!labels.length) return true;
  return labels.some((l) => hasLabel(issue, l));
}

function matchesNoExcludedLabels(issue: LinearIssueLite, exclude: string[]): boolean {
  if (!exclude.length) return true;
  return exclude.every((l) => !hasLabel(issue, l));
}

function inPriorityRange(priority: number | null | undefined, min?: number, max?: number): boolean {
  if (priority === null || priority === undefined) return true;
  if (typeof min === "number" && priority < min) return false;
  if (typeof max === "number" && priority > max) return false;
  return true;
}

export class LinearAdapter {
  private client: LinearClient;
  private cfg: LinearTaskSourceConfig;
  private taskCfg: TaskSourcesConfig;

  private viewerCache?: LinearActor;
  private workflowCacheByTeamId = new Map<string, Map<string, WorkflowStateLite>>();

  constructor(args: { client: LinearClient; linearCfg: LinearTaskSourceConfig; taskCfg: TaskSourcesConfig }) {
    this.client = args.client;
    this.cfg = args.linearCfg;
    this.taskCfg = args.taskCfg;
  }

  static fromTaskSources(taskCfg: TaskSourcesConfig): LinearAdapter {
    if (taskCfg.primary !== "linear") {
      throw new Error(`LinearAdapter requires task_sources.primary="linear"`);
    }
    const linearCfg = taskCfg.linear;
    if (!linearCfg) {
      // Sugestão: valide isso também no loader (se primary=linear, linear é obrigatório)
      throw new Error(`task_sources.linear is required when primary="linear"`);
    }

    const envVar = linearCfg.auth.token_env;
    const token = process.env[envVar];
    if (!token) {
      throw new Error(`Missing Linear token env var: ${envVar}`);
    }

    const client = createLinearClient({ token, endpoint: linearCfg.endpoint });
    return new LinearAdapter({ client, linearCfg, taskCfg });
  }

  /** Quem está autenticado (bom p/ strategy=assignee). */
  async viewer(): Promise<LinearActor> {
    if (this.viewerCache) return this.viewerCache;

    // rawRequest suportado pelo SDK :contentReference[oaicite:8]{index=8}
    const res = await this.client.client.rawRequest<{ viewer: LinearActor }>(GQL.viewer);
    if (!res?.viewer?.id) throw new Error("Linear: viewer() returned no id");
    this.viewerCache = res.viewer;
    return res.viewer;
  }

  /** Busca workflow states do team pra mover issue por nome. */
  private async workflowStatesByName(teamId: string): Promise<Map<string, WorkflowStateLite>> {
    const cached = this.workflowCacheByTeamId.get(teamId);
    if (cached) return cached;

    const res = await this.client.client.rawRequest<{ workflowStates: { nodes: WorkflowStateLite[] } }>(
      GQL.workflowStates,
      { teamId },
    );

    const map = new Map<string, WorkflowStateLite>();
    for (const st of res.workflowStates?.nodes ?? []) {
      map.set(norm(st.name), st);
    }
    this.workflowCacheByTeamId.set(teamId, map);
    return map;
  }

  /** Lista issues "claimable" (Ready + filtros + strategy). */
  async listClaimableIssues(): Promise<LinearIssueLite[]> {
    const readyStates = this.taskCfg.ready_states?.length ? this.taskCfg.ready_states : ["Ready"];
    const claimLabel = this.cfg.claim.label ?? "agent:code247";
    const strategy = this.cfg.claim.strategy;

    // Filtering oficial suporta eq/in e relação por labels.name/state.name etc. :contentReference[oaicite:9]{index=9}
    const filter: any = {
      archivedAt: { null: true },
      state: { name: { in: readyStates } },
    };

    if (this.cfg.filters?.team_key) filter.team = { key: { eq: this.cfg.filters.team_key } };
    if (this.cfg.filters?.project_name) filter.project = { name: { eqIgnoreCase: this.cfg.filters.project_name } };

    // Strategy "label" usa filtro já no servidor (mais eficiente).
    if (strategy === "label" || strategy === "label_or_assignee") {
      filter.labels = { name: { eq: claimLabel } };
    }

    // include_labels (AND com claim label pode ser desejável)
    if (this.cfg.filters?.include_labels?.length) {
      // Linear filtra labels com "at least one label matches" :contentReference[oaicite:10]{index=10}
      // Para múltiplas labels obrigatórias, preferimos filtrar em memória.
      // Aqui só colocamos a primeira como filtro para reduzir universo.
      const first = this.cfg.filters.include_labels[0];
      filter.labels = filter.labels ?? { name: { eq: first } };
    }

    const first = this.cfg.polling?.page_size ?? 50;

    const res = await this.client.client.rawRequest<{
      issues: { nodes: LinearIssueLite[] };
    }>(GQL.issues, {
      first,
      filter,
      orderBy: LinearDocument.PaginationOrderBy.UpdatedAt,
    });

    const issues = res.issues?.nodes ?? [];

    // Filtragem final em memória (pra expressividade e evitar filtros “enganosos” com labels)
    const me = await this.viewer();

    const out = issues
      .filter((i) => inPriorityRange(i.priority ?? null, this.cfg.filters?.min_priority, this.cfg.filters?.max_priority))
      .filter((i) => matchesAnyLabel(i, this.cfg.filters?.include_labels ?? []))
      .filter((i) => matchesNoExcludedLabels(i, this.cfg.filters?.exclude_labels ?? []))
      .filter((i) => {
        if (strategy === "label") return hasLabel(i, claimLabel);
        if (strategy === "assignee") return norm(i.assignee?.id) === norm(me.id);
        // label_or_assignee
        return hasLabel(i, claimLabel) || norm(i.assignee?.id) === norm(me.id);
      })
      // prioridade: maior primeiro; desempate por createdAt antigo primeiro
      .sort((a, b) => {
        const pa = a.priority ?? 0;
        const pb = b.priority ?? 0;
        if (pb !== pa) return pb - pa;
        const ca = Date.parse(a.createdAt ?? "1970-01-01");
        const cb = Date.parse(b.createdAt ?? "1970-01-01");
        return ca - cb;
      });

    return out;
  }

  /** Comenta na issue (Markdown). */
  async comment(issueId: string, body: string): Promise<void> {
    // SDK expõe createComment :contentReference[oaicite:11]{index=11}
    const payload = await this.client.createComment({ issueId, body });
    if (!payload?.success) {
      throw new Error(`Linear: createComment failed for issueId=${issueId}`);
    }
  }

  /** Move issue para um state pelo nome (ex.: "In Progress", "In Review", "Done"). */
  async moveToStateByName(issue: Pick<LinearIssueLite, "id" | "team">, targetStateName: string): Promise<void> {
    const teamId = issue.team?.id;
    if (!teamId) throw new Error(`Linear: issue.team.id missing; cannot move state`);

    const states = await this.workflowStatesByName(teamId);
    const st = states.get(norm(targetStateName));
    if (!st?.id) {
      // dica rápida para debug
      const known = [...states.values()].map((x) => x.name).sort();
      throw new Error(
        `Linear: unknown workflow state "${targetStateName}" for teamId=${teamId}. Known: ${known.join(", ")}`,
      );
    }

    // GraphQL issueUpdate suporta stateId :contentReference[oaicite:12]{index=12}
    const payload = await this.client.updateIssue(issue.id, { stateId: st.id });
    if (!payload?.success) {
      throw new Error(`Linear: updateIssue(stateId) failed for issueId=${issue.id}`);
    }
  }

  /**
   * “Claim” padrão do Code247:
   * - comenta “peguei”
   * - move Ready -> In Progress (nome configurável pelo caller)
   */
  async claim(args: {
    issue: LinearIssueLite;
    inProgressStateName?: string; // default "In Progress"
    commentBody: string;
  }): Promise<void> {
    await this.comment(args.issue.id, args.commentBody);
    await this.moveToStateByName(args.issue, args.inProgressStateName ?? "In Progress");
  }
}
```

* * *

Exemplo de uso (Code247 loop)
-----------------------------

```
import { LinearAdapter } from "./integrations/linear/LinearAdapter";
import type { TaskSourcesConfig } from "./integrations/linear/types";

const taskSources: TaskSourcesConfig = {
  primary: "linear",
  ready_states: ["Ready"],
  linear: {
    auth: { token_env: "LINEAR_API_KEY" },
    claim: { strategy: "label", label: "agent:code247" },
    filters: { team_key: "VVTV" },
    polling: { page_size: 50 },
  },
};

const linear = LinearAdapter.fromTaskSources(taskSources);

const issues = await linear.listClaimableIssues();
const pick = issues[0];
if (pick) {
  await linear.claim({
    issue: pick,
    commentBody: `Peguei esta tarefa ✅\n\nPlano:\n- ...\n- Gates: gate0\n`,
  });
}
```

* * *

Por que esse adapter é “certinho” com a doc oficial
---------------------------------------------------

*   usa o SDK do Linear (`LinearClient`) que suporta **apiKey e OAuth** [Linear](https://linear.app/developers/sdk)
*   usa filtros compatíveis (labels/state/relacionamentos) [Linear](https://linear.app/developers/filtering)
*   usa `issueUpdate(stateId)` (exemplo oficial) [Linear](https://linear.app/developers/graphql?utm_source=chatgpt.com)
*   usa `createComment` (SDK oficial) [Linear](https://linear.app/developers/sdk-fetching-and-modifying-data)
*   usa `rawRequest` quando precisar “furar” o SDK com GraphQL direto [Linear](https://linear.app/developers/advanced-usage)

* * *

Se você quiser, o **\==03** eu faço o parser “Linear Issue description → TaskSpec” (Goal/Scope/DoD/How-to-test) + o “Readiness Gate” automático com comentário pedindo clarificação.

## Prompt:
Sim

## Response:
Thought for 28s

Bora 😄 — **\==03** então é o parser “Issue do Linear → TaskSpec” + readiness gate + comentário automático de clarificação.

Abaixo vai um pacote de código bem direto (sem deps além do que você já usa). Ele:

*   Extrai `Goal / Scope / Non-goals / Acceptance / How to test` de markdown (com variações PT/EN)
*   Suporta headings (`##`), títulos em negrito (`**Goal**:`), e listas/checklists
*   Faz **Readiness Gate** (bloqueia se faltar o obrigatório)
*   Infere `risk` e `touches` por labels + keywords
*   Gera comentário padrão “preenche isso aqui” quando não estiver pronto

* * *

1) `src/tasks/TaskSpec.ts`
--------------------------

```
export type TaskSource = "linear" | "jira" | "github" | "manual";

export type TaskRisk = "cost" | "safety" | "security" | "data" | "ux" | "ops" | "perf";

export type TaskTouches = {
  contracts: boolean;
  policyConfig: boolean;
  infra: boolean;
  userFacing: boolean;
  dataModel: boolean;
};

export type TaskHowToTest = {
  local: string[];
  staging: string[];
  prod: string[];
};

export type TaskSpec = {
  id: string;                 // Linear id (uuid)
  key?: string;               // Linear identifier (e.g. VVTV-123)
  title: string;
  source: TaskSource;
  sourceUrl?: string;

  labels: string[];
  priority?: number | null;

  goal: string;
  scope: string[];
  nonGoals: string[];

  acceptance: string[];       // checkboxes/bullets
  howToTest: TaskHowToTest;

  risk: TaskRisk[];
  touches: TaskTouches;

  // For reporting/debug
  missing?: string[];         // readiness failures
  raw?: {
    description?: string;
  };
};

export type ReadinessResult = {
  executable: boolean;
  missing: string[];
};
```

* * *

2) `src/tasks/markdownParse.ts` (helpers)
-----------------------------------------

```
export function normalizeNewlines(s: string): string {
  return (s ?? "").replace(/\r\n/g, "\n");
}

export function stripHtml(s: string): string {
  // Linear geralmente manda markdown, mas às vezes entra HTML/artefatos.
  return (s ?? "").replace(/<[^>]*>/g, "");
}

export function splitLines(s: string): string[] {
  return normalizeNewlines(stripHtml(s)).split("\n");
}

export function isHeading(line: string): boolean {
  return /^\s*#{1,6}\s+/.test(line.trim());
}

export function headingText(line: string): string {
  return line.trim().replace(/^\s*#{1,6}\s+/, "").trim();
}

export function isBoldLabelLine(line: string): boolean {
  // **Goal**: ...
  return /^\s*\*\*[^*]+\*\*\s*:/.test(line.trim());
}

export function boldLabelKey(line: string): string {
  const m = line.trim().match(/^\s*\*\*([^*]+)\*\*\s*:/);
  return (m?.[1] ?? "").trim();
}

export function boldLabelValue(line: string): string {
  return line.trim().replace(/^\s*\*\*[^*]+\*\*\s*:\s*/, "").trim();
}

export function extractListItems(lines: string[], max = 50): string[] {
  const out: string[] = [];
  const re = /^\s*(?:-|\*|\d+[.)])\s+(.*)$/;

  for (const l of lines) {
    const m = l.match(re);
    if (m?.[1]) {
      const v = m[1].trim();
      if (v) out.push(v);
      if (out.length >= max) break;
    }
  }
  return out;
}

export function extractChecklistItems(lines: string[], max = 50): string[] {
  const out: string[] = [];
  const re = /^\s*[-*]\s*\[(?: |x|X)\]\s+(.*)$/;

  for (const l of lines) {
    const m = l.match(re);
    if (m?.[1]) {
      const v = m[1].trim();
      if (v) out.push(v);
      if (out.length >= max) break;
    }
  }
  return out;
}

export function compactText(lines: string[], maxChars = 600): string {
  const s = lines.map((l) => l.trim()).filter(Boolean).join(" ");
  return s.length > maxChars ? s.slice(0, maxChars).trim() + "…" : s;
}
```

* * *

3) `src/integrations/linear/issueToTaskSpec.ts` (parser + readiness)
--------------------------------------------------------------------

```
import type { LinearIssueLite } from "../linear/types";
import type { ReadinessResult, TaskHowToTest, TaskRisk, TaskSpec, TaskTouches } from "../../tasks/TaskSpec";
import {
  splitLines,
  isHeading,
  headingText,
  isBoldLabelLine,
  boldLabelKey,
  boldLabelValue,
  extractListItems,
  extractChecklistItems,
  compactText,
} from "../../tasks/markdownParse";

type SectionKey = "goal" | "scope" | "nonGoals" | "acceptance" | "howToTest";

const HEADINGS: Record<SectionKey, string[]> = {
  goal: ["goal", "objetivo", "meta"],
  scope: ["scope", "escopo"],
  nonGoals: ["non-goals", "non goals", "fora de escopo", "out of scope", "nao escopo", "não escopo"],
  acceptance: [
    "acceptance",
    "acceptance (dod)",
    "dod",
    "definition of done",
    "criteria",
    "critérios",
    "criterios de aceite",
    "critério de aceite",
    "critério",
  ],
  howToTest: ["how to test", "como testar", "test plan", "plano de teste", "testing"],
};

const SUBHEAD_LOCAL = ["local"];
const SUBHEAD_STAGING = ["staging", "stage", "homolog", "homologação", "hml"];
const SUBHEAD_PROD = ["prod", "production", "produção"];

function norm(s: string): string {
  return (s ?? "").trim().toLowerCase();
}

function looksLikeSectionTitle(title: string, keys: string[]): boolean {
  const t = norm(title);
  return keys.some((k) => t === k || t.startsWith(k + " ") || t.includes(k));
}

function sliceUntilNextHeading(lines: string[], startIndex: number): string[] {
  const out: string[] = [];
  for (let i = startIndex + 1; i < lines.length; i++) {
    if (isHeading(lines[i])) break;
    // Para não capturar um monte de lixo, paramos se achar outro bold-label "section header"
    if (isBoldLabelLine(lines[i])) break;
    out.push(lines[i]);
  }
  return out;
}

function findHeadingSection(lines: string[], keys: string[]): { index: number; title: string } | null {
  for (let i = 0; i < lines.length; i++) {
    if (!isHeading(lines[i])) continue;
    const title = headingText(lines[i]);
    if (looksLikeSectionTitle(title, keys)) return { index: i, title };
  }
  return null;
}

function findBoldLabel(lines: string[], keys: string[]): { index: number; key: string; value: string } | null {
  for (let i = 0; i < lines.length; i++) {
    if (!isBoldLabelLine(lines[i])) continue;
    const k = boldLabelKey(lines[i]);
    if (looksLikeSectionTitle(k, keys)) {
      return { index: i, key: k, value: boldLabelValue(lines[i]) };
    }
  }
  return null;
}

function parseGoal(lines: string[]): string {
  const h = findHeadingSection(lines, HEADINGS.goal);
  if (h) {
    const body = sliceUntilNextHeading(lines, h.index);
    const bullets = extractListItems(body, 5);
    if (bullets.length) return bullets[0];
    const text = compactText(body, 240);
    return text;
  }

  const b = findBoldLabel(lines, HEADINGS.goal);
  if (b) return b.value;

  // fallback: procurar "Goal:" ou "Objetivo:"
  const re = /^\s*(goal|objetivo)\s*:\s*(.+)$/i;
  for (const l of lines) {
    const m = l.match(re);
    if (m?.[2]) return m[2].trim();
  }

  return "";
}

function parseListSection(lines: string[], keys: string[], max = 30): string[] {
  const h = findHeadingSection(lines, keys);
  if (h) {
    const body = sliceUntilNextHeading(lines, h.index);
    const checklist = extractChecklistItems(body, max);
    if (checklist.length) return checklist;
    const bullets = extractListItems(body, max);
    if (bullets.length) return bullets;
    const text = compactText(body, 400);
    return text ? [text] : [];
  }

  const b = findBoldLabel(lines, keys);
  if (b) return b.value ? [b.value] : [];

  return [];
}

function parseHowToTest(lines: string[]): TaskHowToTest {
  const empty: TaskHowToTest = { local: [], staging: [], prod: [] };

  const h = findHeadingSection(lines, HEADINGS.howToTest);
  if (!h) {
    const b = findBoldLabel(lines, HEADINGS.howToTest);
    if (!b) return empty;
    // Se for bold label, a "value" vira um item local genérico
    return { local: b.value ? [b.value] : [], staging: [], prod: [] };
  }

  const body = sliceUntilNextHeading(lines, h.index);
  const bodyLines = body.map((l) => l.trim());

  // Procurar sub-headings dentro do corpo (por headings ### ou por labels "Local:" etc.)
  const buckets: TaskHowToTest = { local: [], staging: [], prod: [] };

  // 1) parse por linhas "Local:" etc.
  const labelRe = /^\s*(local|staging|stage|homolog|homologação|hml|prod|production|produção)\s*:\s*(.+)\s*$/i;
  for (const l of bodyLines) {
    const m = l.match(labelRe);
    if (!m?.[2]) continue;
    const where = norm(m[1]);
    const val = m[2].trim();
    if (!val) continue;

    if (SUBHEAD_LOCAL.includes(where)) buckets.local.push(val);
    else if (SUBHEAD_STAGING.includes(where)) buckets.staging.push(val);
    else if (SUBHEAD_PROD.includes(where)) buckets.prod.push(val);
  }

  // 2) parse listas; se o texto tiver sub-headings como "Local" em linha isolada, segmenta
  const segs = segmentByInlineSubheads(bodyLines);

  if (!buckets.local.length && segs.local.length) buckets.local = segs.local;
  if (!buckets.staging.length && segs.staging.length) buckets.staging = segs.staging;
  if (!buckets.prod.length && segs.prod.length) buckets.prod = segs.prod;

  // 3) fallback: se nada segmentado, pega bullets e joga em local
  if (!buckets.local.length && !buckets.staging.length && !buckets.prod.length) {
    const checklist = extractChecklistItems(body, 30);
    const bullets = checklist.length ? checklist : extractListItems(body, 30);
    if (bullets.length) buckets.local = bullets;
    else {
      const text = compactText(body, 600);
      if (text) buckets.local = [text];
    }
  }

  return buckets;
}

function segmentByInlineSubheads(lines: string[]): TaskHowToTest {
  const buckets: TaskHowToTest = { local: [], staging: [], prod: [] };
  let cur: keyof TaskHowToTest | null = null;

  const isSub = (t: string, keys: string[]) => keys.some((k) => norm(t) === k);

  const flushLineAsItem = (line: string) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    // Se for bullet/checklist, extrair conteúdo
    const chk = trimmed.match(/^\s*[-*]\s*\[(?: |x|X)\]\s+(.*)$/);
    if (chk?.[1]) {
      if (cur) buckets[cur].push(chk[1].trim());
      return;
    }
    const bul = trimmed.match(/^\s*(?:-|\*|\d+[.)])\s+(.*)$/);
    if (bul?.[1]) {
      if (cur) buckets[cur].push(bul[1].trim());
      return;
    }
    if (cur) buckets[cur].push(trimmed);
  };

  for (const l of lines) {
    const t = l.trim();
    if (!t) continue;

    // linhas "Local" / "Staging" / "Prod" sozinhas
    if (isSub(t, SUBHEAD_LOCAL)) { cur = "local"; continue; }
    if (isSub(t, SUBHEAD_STAGING)) { cur = "staging"; continue; }
    if (isSub(t, SUBHEAD_PROD)) { cur = "prod"; continue; }

    // headings markdown dentro da seção (### Local)
    const hm = t.match(/^(#{2,6})\s+(.+)$/);
    if (hm?.[2]) {
      const ht = hm[2].trim();
      if (looksLikeSectionTitle(ht, SUBHEAD_LOCAL)) { cur = "local"; continue; }
      if (looksLikeSectionTitle(ht, SUBHEAD_STAGING)) { cur = "staging"; continue; }
      if (looksLikeSectionTitle(ht, SUBHEAD_PROD)) { cur = "prod"; continue; }
    }

    if (!cur) continue;
    flushLineAsItem(t);
  }

  return buckets;
}

function inferTouches(labels: string[], title: string, desc: string): TaskTouches {
  const L = labels.map(norm);

  const has = (x: string) => L.includes(norm(x)) || L.some((l) => l.startsWith(norm(x) + ":"));

  const text = norm(`${title}\n${desc}`);

  const contracts =
    has("area:schemas") ||
    has("area:sdk") ||
    /schema|openapi|contract|events?\b|registry|payload/.test(text);

  const policyConfig =
    has("area:config") ||
    /config-core|yaml|policy|limits?|threshold|tier|guardrails?/.test(text);

  const infra =
    has("area:infra") || has("area:workers") || has("area:lab") ||
    /wrangler|cloudflare|durable object|r2|worker|deploy|pipeline/.test(text);

  const userFacing =
    has("area:frontend") || /ui|ux|screen|overlay|route|page|frontend/.test(text);

  const dataModel =
    has("area:domain") || /entity|model|domain|migration|db|d1|table/.test(text);

  return { contracts, policyConfig, infra, userFacing, dataModel };
}

function inferRisk(labels: string[], title: string, desc: string, touches: TaskTouches): TaskRisk[] {
  const L = labels.map(norm);
  const risks = new Set<TaskRisk>();

  // explicit labels win
  for (const l of L) {
    if (l.startsWith("risk:")) {
      const v = l.split(":")[1] as TaskRisk;
      if (v) risks.add(v);
    }
  }

  const text = norm(`${title}\n${desc}`);

  if (!risks.size) {
    // inference by keywords / touches
    if (touches.policyConfig || /egress|bitrate|cost|tier|quota|bandwidth/.test(text)) risks.add("cost");
    if (/safety|abuse|report|quarantine|ban|trust|dmca|takedown/.test(text)) risks.add("safety");
    if (/auth|token|jwt|permission|rbac|csrf|xss|security/.test(text)) risks.add("security");
    if (/latency|perf|jitter|packet loss|optimi|cpu|memory/.test(text)) risks.add("perf");
    if (touches.userFacing || /ux|animation|transition|overlay|ios/.test(text)) risks.add("ux");
    if (touches.infra || /deploy|runbook|incident|watchdog|24\/7/.test(text)) risks.add("ops");
    if (touches.dataModel || /migration|schema change|retention/.test(text)) risks.add("data");
  }

  return [...risks];
}

export function issueToTaskSpec(issue: LinearIssueLite): TaskSpec {
  const description = issue.description ?? "";
  const lines = splitLines(description);

  const goal = parseGoal(lines);

  const scope = parseListSection(lines, HEADINGS.scope, 40);
  const nonGoals = parseListSection(lines, HEADINGS.nonGoals, 30);

  // acceptance: prefer checklist, then bullets, then text
  const acceptance = parseListSection(lines, HEADINGS.acceptance, 60);

  const howToTest = parseHowToTest(lines);

  const labels = (issue.labels?.nodes ?? [])
    .map((l) => (l.name ?? "").trim())
    .filter(Boolean);

  const touches = inferTouches(labels, issue.title, description);
  const risk = inferRisk(labels, issue.title, description, touches);

  return {
    id: issue.id,
    key: issue.identifier,
    title: issue.title,
    source: "linear",
    sourceUrl: issue.url ?? undefined,
    labels,
    priority: issue.priority ?? null,

    goal,
    scope,
    nonGoals,
    acceptance,
    howToTest,

    risk,
    touches,

    raw: { description },
  };
}

export function readinessGate(task: TaskSpec, opts?: { requireAcceptance?: boolean; requireHowToTest?: boolean }): ReadinessResult {
  const requireAcceptance = opts?.requireAcceptance ?? true;
  const requireHowToTest = opts?.requireHowToTest ?? true;

  const missing: string[] = [];

  if (!task.goal?.trim()) missing.push("Goal");
  if (requireAcceptance && (!task.acceptance?.length || task.acceptance.every((x) => !x.trim()))) missing.push("Acceptance (DoD)");
  if (requireHowToTest) {
    const hasAnyTest = [...task.howToTest.local, ...task.howToTest.staging, ...task.howToTest.prod].some((x) => x.trim());
    if (!hasAnyTest) missing.push("How to test (Local/Staging/Prod)");
  }

  return { executable: missing.length === 0, missing };
}

export function buildClarificationComment(task: TaskSpec, missing: string[]): string {
  const missingList = missing.map((m) => `- ${m}`).join("\n");

  return [
    `Code247 não consegue executar ainda. Faltam campos obrigatórios na descrição:\n${missingList}\n`,
    `Cole este template e preencha:\n`,
    `## Goal\n(1 frase)\n`,
    `## Acceptance (DoD)\n- [ ] ...\n`,
    `## How to test\n- Local:\n  - ...\n- Staging:\n  - ...\n`,
    `Quando estiver pronto, marque como claimable (ex.: label \`agent:code247\`).`,
  ].join("\n");
}
```

* * *

4) Exemplo de uso no loop do Code247 (Linear → TaskSpec → readiness → comment/claim)
------------------------------------------------------------------------------------

```
import { LinearAdapter } from "../integrations/linear/LinearAdapter";
import { issueToTaskSpec, readinessGate, buildClarificationComment } from "../integrations/linear/issueToTaskSpec";

export async function runOnce(linear: LinearAdapter, projectExecution: { requireHowToTest: boolean; requireAcceptance: boolean }) {
  const issues = await linear.listClaimableIssues();
  const issue = issues[0];
  if (!issue) return;

  const task = issueToTaskSpec(issue);
  const ready = readinessGate(task, {
    requireAcceptance: projectExecution.requireAcceptance,
    requireHowToTest: projectExecution.requireHowToTest,
  });

  if (!ready.executable) {
    await linear.comment(issue.id, buildClarificationComment(task, ready.missing));
    // não move estado, não claim
    return;
  }

  await linear.claim({
    issue,
    commentBody: [
      `Peguei esta tarefa ✅`,
      ``,
      `Goal: ${task.goal}`,
      `Riscos: ${task.risk.join(", ") || "none"}`,
      `Touches: ${Object.entries(task.touches).filter(([, v]) => v).map(([k]) => k).join(", ") || "none"}`,
      ``,
      `Vou considerar DONE apenas com gates passando.`,
    ].join("\n"),
  });

  // daqui em diante: executar ciclo de implementação + gates
}
```

* * *

5) Convenção que deixa isso “mágico” com a pessoa nova
------------------------------------------------------

Vocês só precisam combinar uma regra simples de claim:

*   tarefas do bot: **label `agent:code247`**
*   tarefas humanas: sem label (ou label `agent:human`)

O resto vira automático: o Code247 _puxa_, valida readiness, comenta template se faltar, ou segue.

* * *

Se você quiser, o **\==04** é deixar isso ainda mais suave:

*   Parser também lê `Acceptance` como checklists em qualquer lugar (mesmo sem heading)
*   Detector de “touches” por diff (quando já tem branch)
*   Comentário automático com “labels sugeridas” (risk/area) baseado na inferência
