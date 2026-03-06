# **LOG.LINE.WORLD DNS + INTEGRATION PLAN (Linear + GitHub App)**

Updated: 2026-03-05

## **1) What is already set now**

### **DNS records created in Cloudflare (zone `logline.world`)**

All records below are `CNAME` -> `975af37c-74a5-4995-992e-e9b0dba02322.cfargotunnel.com` with proxy enabled:

- `code247.logline.world`
- `llm-gateway.logline.world`
- `obs-api.logline.world`
- `obs.logline.world` (alias)
- `edge-control.logline.world`

### **Tunnel ingress updated locally (`~/.cloudflared/config.yml`)**

- `llm-gateway.logline.world` -> `http://127.0.0.1:7700`
- `code247.logline.world` -> `http://127.0.0.1:4001`
- `obs-api.logline.world` -> `http://127.0.0.1:3000`
- `obs.logline.world` -> `http://127.0.0.1:3000`
- `edge-control.logline.world` -> `http://127.0.0.1:18080`

Notes:
- `cloudflared-logline` process is running in PM2.
- `18080` was chosen to avoid conflict with an existing local daemon on `8080`.

## **2) Canonical public URLs to use from now on**

### **Ecosystem services**

- `https://code247.logline.world`
- `https://llm-gateway.logline.world`
- `https://obs-api.logline.world`
- `https://edge-control.logline.world`

### **Linear integration URLs**

- OAuth start: `https://code247.logline.world/oauth/start`
- OAuth callback: `https://code247.logline.world/oauth/callback`
- OAuth status: `https://code247.logline.world/oauth/status`
- Intended webhook target: `https://code247.logline.world/webhooks/linear` (implementation pending)

### **GitHub App integration URLs**

- Webhook target (recommended): `https://edge-control.logline.world/v1/orchestrate/github-event`
- App homepage: `https://code247.logline.world`
- Optional user OAuth callback (if enabled): `https://code247.logline.world/oauth/github/callback`

## **3) GitHub App spec (what you need to fill)**

Create one GitHub App for Code247 automation with:

### **General**

- App name: `Code247`
- Homepage URL: `https://code247.logline.world`
- Webhook URL: `https://edge-control.logline.world/v1/orchestrate/github-event`
- Webhook secret: generate a strong random secret (store in Doppler as `GITHUB_WEBHOOK_SECRET`)

### **Permissions (minimum practical)**

Repository permissions:
- Contents: `Read and write`
- Pull requests: `Read and write`
- Issues: `Read and write`
- Metadata: `Read-only`
- Commit statuses: `Read and write`

Organization permissions:
- Members: `Read-only` (optional)

### **Subscribe to events**

- `pull_request`
- `push`
- `issue_comment`
- `check_run`
- `check_suite`
- `installation`
- `installation_repositories`

### **Doppler keys for GitHub App mode**

- `GITHUB_APP_ID`
- `GITHUB_APP_PRIVATE_KEY`
- `GITHUB_APP_INSTALLATION_ID`
- `GITHUB_WEBHOOK_SECRET`

Fallback mode (temporary):
- `GITHUB_TOKEN`

## **4) Linear OAuth spec (what you need to fill)**

In Linear (Admin -> API -> OAuth Applications):

- Redirect URI: `https://code247.logline.world/oauth/callback`
- Scopes: `read write comments:create issues:create`
- Actor mode: `actor=app`

Doppler keys:
- `LINEAR_CLIENT_ID`
- `LINEAR_CLIENT_SECRET`
- `LINEAR_TEAM_ID`
- Optional fallback: `LINEAR_API_KEY`

## **5) Immediate operational checklist**

1. Keep temporary mode running:
   - Linear via `LINEAR_API_KEY + LINEAR_TEAM_ID`
   - GitHub via temporary `GITHUB_TOKEN` **or** move directly to GitHub App keys.
2. Start ecosystem services on their canonical ports (`7700/4001/3000/18080`).
3. Validate external health:
   - `https://code247.logline.world/health`
   - `https://llm-gateway.logline.world/health`
   - `https://obs-api.logline.world/api/health`
   - `https://edge-control.logline.world/health`
4. Enable OAuth in Linear once callback is reachable.
5. Configure GitHub App webhook once edge-control webhook path is active.

## **6) Gap to close before production webhooks**

- `code247` Linear webhook endpoint (`/webhooks/linear`) still needs final hardening path in runtime (signature + replay + idempotency).
- GitHub webhook route currently needs final signature-validation flow (public webhook ingress + deterministic internal orchestration).

