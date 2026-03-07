# Cookbook Canônico (Gerado)

Gerado em: `2026-03-07T19:38:40.557Z`
Fonte: `contracts/generated/capability-catalog.v1.json`

## Resumo
- Total de capacidades: **131**
- CLI: **78**
- API: **53**

## Capacidades CLI

| Comando | Resumo |
|---|---|
| `logline app config-export` | Export ecosystem config JSON for an app to consume |
| `logline app create` | Register a new app under the current tenant |
| `logline app handshake` | Bidirectional handshake: store the app's service URL and API key |
| `logline app issue-service-token` | Issue a long-lived service token for app-to-app authentication |
| `logline app list` | List apps in the current tenant |
| `logline app list-service-tokens` | List service tokens for an app |
| `logline app revoke-service-token` | Revoke a service token |
| `logline app trust` | Mark an app as trusted (can use billing delegation) |
| `logline auth debug` | Debug auth/RLS - show JWT claims and test PostgREST |
| `logline auth lock` | Lock session immediately (revoke access) |
| `logline auth login` | Login with email/password (Supabase Auth direct) |
| `logline auth logout` | Remove stored tokens and logout |
| `logline auth passkey-register` | Register a passkey (Ed25519 keypair + Touch ID gate) |
| `logline auth status` | Show session status and remaining TTL |
| `logline auth unlock` | Unlock session with Touch ID (required before any privileged command) |
| `logline auth whoami` | Show current identity |
| `logline backend list` | - |
| `logline backend test` | - |
| `logline broadcast send` | Send a message to a realtime channel |
| `logline catalog export` | Export CLI command catalog as JSON |
| `logline cicd run` | Run the CI/CD pipeline defined in logline.cicd.json |
| `logline cicd status` | Show the status of the last pipeline run |
| `logline db describe` | Describe a table: columns, types, constraints |
| `logline db migrate apply` | Apply pending migrations (requires recent review receipt + infra identity) |
| `logline db migrate review` | Review pending migrations (generates diff, stores review receipt) |
| `logline db migrate status` | Show migration status (applied vs pending) |
| `logline db migrate up` | [Legacy] Apply all pending migrations without review gate |
| `logline db query` | Execute a SQL query and return results as JSON |
| `logline db tables` | List all tables with row counts |
| `logline db verify-rls` | Verify RLS is enabled and policies exist on all tables (mandatory gate) |
| `logline deploy all` | Deploy everything: Supabase -> GitHub -> Vercel (with gates) |
| `logline deploy github` | Deploy to GitHub: git push (and optionally create PR or release) |
| `logline deploy supabase` | Deploy Supabase: run migrations + verify RLS |
| `logline deploy vercel` | Deploy to Vercel: sync env vars + poll deployment |
| `logline dev build` | Build the Next.js app (npm run build with DATABASE_URL injected) |
| `logline dev migrate-generate` | Generate Drizzle migration files (drizzle-kit generate with creds injected) |
| `logline dev migrate-push` | Push Drizzle schema to database (drizzle-kit push with creds injected) |
| `logline dev start` | Start the dev server (npm run dev with DATABASE_URL injected) |
| `logline events` | - |
| `logline founder bootstrap` | One-time world bootstrap (creates tenant, user, memberships, founder cap) |
| `logline fuel emit` | Emit a fuel event |
| `logline fuel list` | List fuel events (optionally filtered) |
| `logline fuel summary` | Get fuel summary (totals by app/unit_type) |
| `logline harness cookbook` | Generate cookbook from the canonical capability catalog |
| `logline harness intentions generate` | Generate a canonical manifest.intentions.json for logic/CLI release flow |
| `logline harness intentions publish` | Publish manifest.intentions.json through Code247 intake and persist linkage meta |
| `logline harness intentions replay` | Replay sync from a previous harness report (requires sync payload path in report) |
| `logline harness intentions sync` | Sync execution status to Code247/Linear using a payload file |
| `logline harness replay` | Replay a previous severe integration report (all scenarios or only failed ones) |
| `logline harness run` | Run the unified severe integration pipeline and write an auditable report |
| `logline init` | - |
| `logline onboard` | Interactive wizard to onboard a new app into the ecosystem |
| `logline profile list` | - |
| `logline profile use` | - |
| `logline ready` | Pre-flight check: vault + session + identity + pipeline readiness |
| `logline run` | - |
| `logline secrets clear` | Remove ALL stored credentials |
| `logline secrets doctor` | Check vault completeness against pipeline requirements |
| `logline secrets get` | Retrieve a credential from Keychain (requires unlocked session) |
| `logline secrets ls` | List all stored credential keys (names only, never values) |
| `logline secrets rm` | Remove a credential from Keychain |
| `logline secrets set` | Store a credential in macOS Keychain (prompted, never echoed) |
| `logline status` | - |
| `logline stop` | - |
| `logline storage delete` | Delete a file from storage |
| `logline storage download` | Download a file from storage |
| `logline storage list` | List files in a bucket |
| `logline storage sign` | Generate a signed URL for a file |
| `logline storage upload` | Upload a file to storage |
| `logline supabase check` | - |
| `logline supabase link` | - |
| `logline supabase migrate` | - |
| `logline supabase projects` | - |
| `logline supabase raw` | - |
| `logline supabase store-token` | Store Supabase access token in OS keychain (never on disk) |
| `logline tenant allowlist-add` | Add an email to the tenant allowlist |
| `logline tenant create` | Create a new tenant (founder only) |
| `logline tenant resolve` | Resolve tenant by slug |

## Capacidades API por Serviço

### code247

| Método | Path | Resumo |
|---|---|---|
| `GET` | `/health` | Health check |
| `GET` | `/intentions/{workspace}/{project}` | Read latest linkage snapshot for workspace/project |
| `GET` | `/jobs` | List recent jobs |
| `GET` | `/oauth/callback` | Linear OAuth callback |
| `GET` | `/oauth/start` | Start Linear OAuth flow |
| `GET` | `/oauth/status` | OAuth configuration/connection status |
| `POST` | `/intentions` | Ingest intentions and sync to Linear |
| `POST` | `/intentions/sync` | Sync execution status/evidence back to Linear |
| `POST` | `/jobs` | Create job |
| `POST` | `/webhooks/linear` | Receive Linear webhook deliveries |

### edge-control

| Método | Path | Resumo |
|---|---|---|
| `GET` | `/health` | Health check |
| `POST` | `/v1/fuel/diff/route` | Produce OpinionSignal from deterministic fuel diff pack |
| `POST` | `/v1/intention/draft` | Generate DraftIntention from free text |
| `POST` | `/v1/orchestrate/github-event` | Sync GitHub lifecycle events back to Linear through Code247 |
| `POST` | `/v1/orchestrate/intention-confirmed` | Checkpoint YES/HUMAN #1 and dispatch intention to Code247/Linear |
| `POST` | `/v1/orchestrate/rollback` | Checkpoint YES/HUMAN #2 and sync rollback outcome |
| `POST` | `/v1/pr/risk` | Produce OpinionSignal for PR risk |

### inference-plane

| Método | Path | Resumo |
|---|---|---|
| `GET` | `/health` | Health check |
| `POST` | `/v1/embed` | Generate embeddings for deterministic docs |
| `POST` | `/v1/fuel/diff/route` | Emit opinion from fuel snapshot diff pack |
| `POST` | `/v1/intention/draft` | Draft intention AST and rollback plan |
| `POST` | `/v1/pr/risk` | Emit PR risk opinion |

### llm-gateway

| Método | Path | Resumo |
|---|---|---|
| `GET` | `/health` | Health check |
| `GET` | `/healthz` | Health check alias |
| `GET` | `/v1/contracts/code247` | Gateway to Code247 formal contract |
| `GET` | `/v1/llm/matrix` | Get model capability matrix |
| `GET` | `/v1/metrics/summary` | Metrics summary (JSON) |
| `GET` | `/v1/models` | List available models |
| `GET` | `/v1/modes` | Mode contract |
| `POST` | `/v1/admin/fuel/reconcile/cloud` | Reconcile settled cloud cost (OpenAI/Anthropic) for a window |
| `POST` | `/v1/admin/release/manifest` | Publish manifest.intentions.json for release sync |
| `POST` | `/v1/chat/completions` | Create chat completion |

### obs-api

| Método | Path | Resumo |
|---|---|---|
| `GET` | `/api/v1/alerts/open` | List open alerts |
| `GET` | `/api/v1/apps/{appId}/keys/user` | List user-owned provider keys for app/tenant |
| `GET` | `/api/v1/auth/whoami` | Resolve current user and memberships |
| `GET` | `/api/v1/capabilities/catalog` | Get canonic capability catalog for CLI + APIs |
| `GET` | `/api/v1/code247/run-timeline` | Get Code247 operational run timeline |
| `GET` | `/api/v1/code247/stage-telemetry` | Get Code247 stage latency and cost telemetry |
| `GET` | `/api/v1/dashboards/summary` | Get cross-app dashboard summary |
| `GET` | `/api/v1/fuel/alerts` | Materialize and list open Fuel alerts |
| `GET` | `/api/v1/fuel/calibration` | Get Fuel calibration analysis for k_latency, k_errors and k_energy |
| `GET` | `/api/v1/fuel/dashboard` | Get Fuel dashboard payload (Realtime + Estatistica) |
| `GET` | `/api/v1/fuel/ops` | Get recurring Fuel baseline and alert job evidence |
| `GET` | `/api/v1/fuel/reconciliation` | Get Fuel reconciliation payload (coverage + drift + segmentação) |
| `GET` | `/api/v1/runs/{runId}` | Get projected run state |
| `GET` | `/api/v1/timeline/{intentionId}` | Get timeline by intention |
| `GET` | `/api/v1/traces/{traceId}` | Get causal trace tree |
| `POST` | `/api/v1/alerts/ack` | Acknowledge alert |
| `POST` | `/api/v1/apps/{appId}/keys/user` | Create user-owned provider key |
| `POST` | `/api/v1/auth/onboard/claim` | Claim memberships from allowlist |
| `POST` | `/api/v1/auth/tenant/resolve` | Resolve tenant by slug |
| `POST` | `/api/v1/events/ingest` | Ingest operational event |
| `POST` | `/api/v1/fuel/ops` | Materialize recurring Fuel baseline and alert jobs |

## Uso Operacional
- Atualizar catálogo: `node scripts/generate-capability-catalog.mjs`
- Atualizar cookbook: `node scripts/generate-cookbook.mjs`
- Pipeline severo: `./scripts/integration-severe.sh`

> Este arquivo é gerado automaticamente. Não editar manualmente.

