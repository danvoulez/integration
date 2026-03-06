# Code247 Intention Intake — manifest + ingestion contract

Code247 acts both as builder and as maintenance infrastructure: it receives the latest intentions (project + task manifests) from each service, converts them into Linear issues, and feeds CI/automation downstream. This document describes the manifest schema every project should emit and the official `POST /intentions` ingestion endpoint that Code247 exposes.

## 1. Manifest schema (publisher side)
Each project keeps a JSON/YAML manifest (e.g., `manifest.intentions.json` next to `TASKLIST.md`) with three sections:

```yaml
workspace: logline.world            # unique workspace identifier
project: logic.logline.world        # project slug / linear team key
updated_at: '2026-03-04T12:00:00Z'
intentions:
  - id: LOGIC-ENT-101
    title: Harden auth policies
    type: project
    scope: infra
    priority: high
    tasks:
      - description: ensure auth hook RPC access is revoked
        owner: supabase-migrations
        due: '2026-03-10'
        gate: security
      - description: add audit log for role changes
        owner: logline-auth
        gate: audit
  - id: LOGIC-ENT-102
    title: Linear integration proof-of-concept
    type: backlog
    priority: medium
    tasks: []
```

Fields:

| Field | Description |
|-------|-------------|
| `workspace` | canonical workspace id used by Linear/legal gating. |
| `project` | service slug used to select the Linear board and CircleCI job. |
| `updated_at` | ISO timestamp. Code247 rejects older updates if newer manifest already ingested. |
| `intentions` | array of intention records (one per feature/phase). |
| `intention.id` | unique code, reused when manifest changes to update Linear issue. |
| `intention.title` | summary that becomes the Linear issue title. |
| `type` | `project|gate|epic|backlog` (used for templating). |
| `scope` | optional domain label (security, discovery, frontend, etc.). |
| `priority` | low/medium/high/critical. |
| `tasks` | list of actionable check items, each with `description`, `owner`, `due`, `gate`. |

Any missing field defaults to `null`. The manifest is intentionally small so projects can update it manually or via scripts.

## 2. Ingestion contract (`POST /intentions`)
Code247 exposes an authenticated endpoint that every project calls when its manifest changes. It validates the payload, creates/updates the matching Linear issue tree, and replies with the issue IDs and CI job names to run.

### Request
- `POST /intentions`
- Headers: `Authorization: Bearer <code247-token>`, `Content-Type: application/json`
- Body (JSON):

```json
{
  "manifest": { ... },
  "source": "logic.logline.world",
  "revision": "a1b2c3d",
  "ci_target": "logic-ci/main"
}
```

Fields:

| Field | Description |
|-------|-------------|
| `manifest` | The manifest schema above. |
| `source` | Git repo path or identifier. Used for traceability. |
| `revision` | Git commit or artifact id. Code247 stores it for reproducible builds. |
| `ci_target` | CI job identifier to trigger (e.g., `logic-ci/main`, `vvv-front/deploy`). |

### Response
- `200 OK` with JSON:

```json
{
  "linear": {
    "intentions": [
      { "id": "LOGIC-ENT-101", "issue_id": "LI-342", "board": "Logic" }
    ]
  },
  "ci": {
    "jobs": ["logic-ci/main"],
    "queue_id": "q-789"
  }
}
```

If validation fails, Code247 returns `400` with `{ request_id, error: { code, message, details } }` following the same envelope used elsewhere.

### Processing guarantees
1. Idempotent: repeated manifest posts with the same `updated_at` are deduped. Change detection uses `(project, intention.id)` pair.
2. Linear synchronization: Code247 ensures each intention maps to a Linear issue (creates if missing, updates if exists). `tasks` become checklist items inside the issue. The response returns the issue IDs for tracing.
3. CI trigger: after the Linear update, Code247 queues the requested `ci_target`; the response includes the queue identifier.
4. Observability: each ingestion logs `workspace`, `project`, `source`, `revision`, and `updated_at` in the event store.

### Readback (round-trip)
- `GET /intentions/{workspace}/{project}` (Bearer required)
- Returns latest ingestion metadata + current mapping `intention.id -> Linear issue`.
- Intended for project-side reconciliation and to rebuild `.code247/linear-meta.json` when needed.

### Execution sync back to Linear
- `POST /intentions/sync` (Bearer required)
- Used by CI/executor after run completion to post evidence and optionally move issue to Done.
- Body:

```json
{
  "workspace": "logline.world",
  "project": "logic.logline.world",
  "results": [
    {
      "intention_id": "LOGIC-ENT-101",
      "status": "success",
      "summary": "CI + gates passed",
      "ci": { "queue_id": "q-123", "job": "logic-ci/main", "url": "https://ci.example/job/123" },
      "evidence": [{ "label": "proof_report", "url": "https://obs.example/reports/123" }],
      "set_done_on_success": true
    }
  ]
}
```

## 3. Recommended workflow
1. Each project detects manifest changes (git commit, manual edit, scheduling).  
2. It POSTs the manifest to Code247 (`/intentions`).  
3. Code247 validates, updates Linear, triggers the relevant CI, and returns the Linear IDs plus job queuing info.  
4. The project stores the Linear IDs next to the manifest (e.g., in `.code247/linear-meta.json`) so future patches know which issues to update.

## 4. Security & trust
- The endpoint uses `code247-token` tied to the project. Rotate tokens via the runbook in `docs/runbook.md`.  
- Only the owning service may post for its `project` slug; mismatched workspace/project combinations are rejected.  
- `revision` ensures auditors can replay the source commit that demanded the intention.  
- All responses include `request_id` for audit trails.

## 5. Next steps
- Implement a tiny client helper (shell script or Rust/TS) that reads `manifest.intentions.json` + `TASKLIST.md` and pushes it to Code247 after each update.  
- Extend the Linear intake to accept the new manifest fields and emit onboarding warnings when a project is missing the manifest.  
- Once the CI job runs, the automation updates the Linear issue with the job result so you can see “what happened to my intention” all in one place.
