# Service Topology

Version: 1.0.0
Date: 2026-03-02
Status: Active
Parent: `INTEGRATION_BLUEPRINT.md`

---

## 1) Network Topology

### 1.1 Physical Layout

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              INTERNET                                           │
│                                                                                 │
│  Clients ──► Cloudflare Edge ──► Cloudflare Tunnel ──► Local Services          │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          OPERATOR MACHINE (macOS)                               │
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                         cloudflared (PM2)                               │    │
│  │                                                                         │    │
│  │  ingress:                                                               │    │
│  │    llm-gateway.logline.world ──► http://127.0.0.1:7700                  │    │
│  │    obs-api.logline.world ──────► http://127.0.0.1:3001                  │    │
│  │    code247.logline.world ──────► http://127.0.0.1:4001                  │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                        │                                        │
│       ┌────────────────────────────────┼────────────────────────────────┐       │
│       │                                │                                │       │
│       ▼                                ▼                                ▼       │
│  ┌─────────────┐              ┌─────────────┐              ┌─────────────┐      │
│  │ llm-gateway │              │   code247   │              │   obs-api   │      │
│  │   :7700     │◄─────────────┤   :4001     │              │   :3001     │      │
│  │   (Rust)    │  LLM calls   │   (Rust)    │              │  (Next.js)  │      │
│  └──────┬──────┘              └──────┬──────┘              └──────┬──────┘      │
│         │                            │                            │             │
│         │                            │                            │             │
│         ▼                            ▼                            ▼             │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │                         SUPABASE (Cloud)                                │    │
│  │  https://aypxnwofjtdnmtxastti.supabase.co                               │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          LOCAL NETWORK (LAN)                                    │
│                                                                                 │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                        │
│  │   LAB-256   │     │   LAB-512   │     │   LAB-8GB   │                        │
│  │ 192.168.0. │     │  localhost  │     │ 192.168.0. │                        │
│  │    125     │     │   :11434    │     │    199     │                        │
│  │   Ollama   │     │   Ollama    │     │   Ollama   │                        │
│  │ llama3.2:3b│     │ qwen2.5:3b  │     │qwen2.5-coder│                       │
│  └─────────────┘     └─────────────┘     └─────────────┘                        │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Port Allocation

| Port | Service | Protocol | Binding | External |
|------|---------|----------|---------|----------|
| 7700 | llm-gateway | HTTP | 127.0.0.1 | Via tunnel |
| 3001 | obs-api | HTTP | 127.0.0.1 | Via tunnel |
| 4001 | code247 | HTTP | 127.0.0.1 | Via tunnel |
| 11434 | Ollama (local) | HTTP | 127.0.0.1 | No |
| 11434 | Ollama (LAB-256) | HTTP | 192.168.0.125 | LAN only |
| 11434 | Ollama (LAB-8GB) | HTTP | 192.168.0.199 | LAN only |

### 1.3 DNS Records

| Hostname | Type | Target | Proxy |
|----------|------|--------|-------|
| `llm-gateway.logline.world` | CNAME | `<tunnel-uuid>.cfargotunnel.com` | Cloudflare |
| `obs-api.logline.world` | CNAME | `<tunnel-uuid>.cfargotunnel.com` | Cloudflare |
| `code247.logline.world` | CNAME | `<tunnel-uuid>.cfargotunnel.com` | Cloudflare |
| `logic.logline.world` | A/CNAME | (documentation only) | N/A |

---

## 2) Service Communication Matrix

### 2.1 Who Calls Whom

```
                    ┌──────────────┐
                    │   External   │
                    │   Clients    │
                    └──────┬───────┘
                           │ HTTPS (via CF)
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
    ┌───────────┐    ┌───────────┐    ┌───────────┐
    │llm-gateway│    │  obs-api  │    │  code247  │
    └─────┬─────┘    └─────┬─────┘    └─────┬─────┘
          │                │                │
          │                │                │
          ▼                ▼                ▼
    ┌─────────────────────────────────────────────┐
    │              Supabase (Auth/DB)             │
    └─────────────────────────────────────────────┘
          │                │                │
          ▼                │                │
    ┌───────────┐          │                │
    │  Ollama   │          │                ▼
    │  (LAN)    │          │         ┌───────────┐
    └───────────┘          │         │llm-gateway│
                           │         │ (internal)│
                           ▼         └───────────┘
                    ┌───────────┐
                    │  Linear   │
                    │   API     │
                    └───────────┘
```

### 2.2 Communication Matrix

| From | To | Protocol | Auth | Purpose |
|------|-----|----------|------|---------|
| Client | llm-gateway | HTTPS | JWT/API key | LLM completions |
| Client | obs-api | HTTPS | JWT | Dashboard UI |
| Client | code247 | HTTPS | JWT | Job management |
| obs-api | llm-gateway | HTTP | API key | LLM proxy |
| obs-api | Supabase | HTTPS | Service key | DB access |
| code247 | llm-gateway | HTTP | API key | LLM calls |
| code247 | Supabase | HTTPS | Service key | Jobs/fuel |
| code247 | Linear | HTTPS | API key | Issue management |
| llm-gateway | Supabase | HTTPS | Service key | Fuel/telemetry |
| llm-gateway | Ollama | HTTP | None | LLM inference |
| CLI | Supabase | HTTPS | JWT | All operations |

### 2.3 Internal vs External URLs

| Service | Internal URL | External URL |
|---------|--------------|--------------|
| llm-gateway | `http://localhost:7700` | `https://llm-gateway.logline.world` |
| obs-api | `http://localhost:3001` | `https://obs-api.logline.world` |
| code247 | `http://localhost:4001` | `https://code247.logline.world` |
| Supabase | N/A | `https://aypxnwofjtdnmtxastti.supabase.co` |
| Ollama (local) | `http://localhost:11434` | N/A |
| Ollama (LAB-256) | `http://192.168.0.125:11434` | N/A |
| Ollama (LAB-8GB) | `http://192.168.0.199:11434` | N/A |

**Rule:** Services on the same host MUST use internal URLs.

---

## 3) Ollama Routing

### 3.1 Model Distribution

| Route Name | Host | Model | VRAM | Use Case |
|------------|------|-------|------|----------|
| `lab-512` | localhost:11434 | qwen2.5:3b | 512MB | Fast/default |
| `lab-256` | 192.168.0.125:11434 | llama3.2:3b | 256MB | Background |
| `lab-8gb` | 192.168.0.199:11434 | qwen2.5-coder:7b | 8GB | Code tasks |

### 3.2 Routing Logic

```
Request arrives at llm-gateway
        │
        ▼
┌───────────────────┐
│  Parse mode/hint  │
└─────────┬─────────┘
          │
    ┌─────▼─────┐
    │ mode=auto │──────► Route by task_hint
    └───────────┘              │
          │              ┌─────▼─────┐
    ┌─────▼─────┐        │ planning  │──► premium (if available) or lab-8gb
    │mode=local │        │ coding    │──► lab-8gb
    └─────┬─────┘        │ review    │──► premium (if available) or lab-8gb
          │              │ background│──► lab-256 or lab-512
          ▼              └───────────┘
    Try routes in order:
    1. lab-8gb (if model matches)
    2. lab-512 (default)
    3. lab-256 (fallback)
```

### 3.3 Health Checks

llm-gateway polls each route every 30 seconds:

```
GET http://<host>:11434/api/tags
```

Routes marked unhealthy after 3 consecutive failures.
Cooldown: 45 seconds before retry.

---

## 4) Supabase Connectivity

### 4.1 Project Details

| Property | Value |
|----------|-------|
| Project URL | `https://aypxnwofjtdnmtxastti.supabase.co` |
| API URL | `https://aypxnwofjtdnmtxastti.supabase.co/rest/v1` |
| Auth URL | `https://aypxnwofjtdnmtxastti.supabase.co/auth/v1` |
| Realtime URL | `wss://aypxnwofjtdnmtxastti.supabase.co/realtime/v1` |
| Storage URL | `https://aypxnwofjtdnmtxastti.supabase.co/storage/v1` |
| JWKS URL | `https://aypxnwofjtdnmtxastti.supabase.co/auth/v1/.well-known/jwks.json` |

### 4.2 Connection Types

| Service | Connection Type | Key Used |
|---------|-----------------|----------|
| obs-api | Direct Postgres | `DATABASE_URL` |
| llm-gateway | PostgREST | Service key |
| code247 | PostgREST | Service key |
| CLI | PostgREST | JWT (user) |
| Client (browser) | PostgREST | Anon key + JWT |

### 4.3 Realtime Channels

| Channel Pattern | Publisher | Subscribers | Payload |
|-----------------|-----------|-------------|---------|
| `code247:jobs:{tenant_id}` | code247 | obs-api | Job status updates |
| `gateway:health` | llm-gateway | obs-api | Provider health |
| `fuel:events:{tenant_id}` | Any | obs-api | New fuel events |

---

## 5) Failure Domains

### 5.1 Single Points of Failure

| Component | Impact if Down | Mitigation |
|-----------|----------------|------------|
| Cloudflare Tunnel | All external access | PM2 auto-restart |
| Supabase | All persistence/auth | None (SaaS dep) |
| Operator machine | All services | None (single node) |
| Internet | External + Supabase | LAN Ollama still works |

### 5.2 Graceful Degradation

| Scenario | Behavior |
|----------|----------|
| Supabase down | Gateway falls back to SQLite cache, reject new users |
| Premium APIs down | Auto-mode routes to local Ollama |
| All Ollama down | Return 503 with retry headers |
| Linear down | Jobs queue locally, retry on recovery |

---

## 6) Security Boundaries

### 6.1 Trust Zones

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              UNTRUSTED ZONE                                     │
│                                                                                 │
│  • Internet traffic                                                             │
│  • Client-provided headers (x-user-id, x-workspace-id)                          │
│  • Unverified JWTs                                                              │
│                                                                                 │
└────────────────────────────────────────┬────────────────────────────────────────┘
                                         │
                              ┌──────────▼──────────┐
                              │  Cloudflare Edge    │
                              │  (DDoS, WAF, TLS)   │
                              └──────────┬──────────┘
                                         │
┌────────────────────────────────────────▼────────────────────────────────────────┐
│                              SEMI-TRUSTED ZONE                                  │
│                                                                                 │
│  • Cloudflare Tunnel (authenticated tunnel)                                     │
│  • Requests with valid JWT (verified signature)                                 │
│  • API key authenticated requests                                               │
│                                                                                 │
└────────────────────────────────────────┬────────────────────────────────────────┘
                                         │
┌────────────────────────────────────────▼────────────────────────────────────────┐
│                              TRUSTED ZONE                                       │
│                                                                                 │
│  • Local services (127.0.0.1 binding)                                           │
│  • Service-to-service calls on localhost                                        │
│  • Supabase service key operations                                              │
│  • PM2 process management                                                       │
│  • macOS Keychain access                                                        │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Port Exposure Rules

- **MUST bind to 127.0.0.1**: All services (llm-gateway, code247, obs-api)
- **MUST NOT bind to 0.0.0.0**: Prevents direct LAN access bypassing tunnel
- **Exception**: Ollama on LAN machines (192.168.x.x) for dedicated inference

---

## 7) Monitoring Points

### 7.1 Health Endpoints

| Service | Endpoint | Expected Response |
|---------|----------|-------------------|
| llm-gateway | `GET /health` | `{"status": "ok", ...}` |
| code247 | `GET /health` | `{"status": "ok", "engine": "rust"}` |
| obs-api | `GET /api/health` | `{"ok": true}` |
| Ollama | `GET /api/tags` | `{"models": [...]}` |

### 7.2 PM2 Monitoring

```bash
# Service status
pm2 status

# Service logs
pm2 logs <service> --lines 50

# Service metrics
pm2 monit

# Restart service
pm2 restart <service>
```

### 7.3 Cloudflare Monitoring

```bash
# Tunnel status
cloudflared tunnel info logline

# Tunnel logs
pm2 logs cloudflared --lines 50
```

---

## References

- `INTEGRATION_BLUEPRINT.md` — Master integration document
- `INFRA_RUNBOOK.md` — Operational procedures
- `llm-gateway.logline.world/openapi.yaml` — Gateway API spec
