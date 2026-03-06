# Infrastructure Runbook

Version: 1.0.0
Date: 2026-03-02
Status: Active
Parent: `INTEGRATION_BLUEPRINT.md`

---

## 1) Overview

This runbook covers daily operations for the LogLine ecosystem infrastructure:
- PM2 process management
- Cloudflare Tunnel
- Service lifecycle
- Incident response

**Host:** Operator machine (macOS)
**Process Manager:** PM2
**Tunnel:** Cloudflare Tunnel (`cloudflared`)

---

## 2) PM2 Operations

### 2.1 Configuration File

Location: `/Users/ubl-ops/Integration/ecosystem.config.cjs` (auto-generated)

Load/reload configuration:
```bash
cd /Users/ubl-ops/Integration
pm2 startOrReload ecosystem.config.cjs
```

Regenerate PM2 + cloudflared configs from single source of truth:
```bash
cd /Users/ubl-ops/Integration
node scripts/generate-topology-configs.mjs --apply-home
```

### 2.2 Daily Commands

```bash
# ─── Status ───────────────────────────────────────────────────────────────────
pm2 status                          # Overview of all services
pm2 show <service>                  # Detailed info for one service
pm2 monit                           # Real-time dashboard (interactive)

# ─── Logs ─────────────────────────────────────────────────────────────────────
pm2 logs                            # All logs (streaming)
pm2 logs <service>                  # Single service logs
pm2 logs <service> --lines 50      # Last 50 lines
pm2 logs <service> --nostream      # Non-streaming (snapshot)
pm2 flush                           # Clear all log files

# ─── Lifecycle ────────────────────────────────────────────────────────────────
pm2 start <service>                 # Start service
pm2 stop <service>                  # Stop service
pm2 restart <service>               # Restart service
pm2 reload <service>                # Graceful reload
pm2 delete <service>                # Remove from PM2

# ─── Ecosystem ────────────────────────────────────────────────────────────────
pm2 startOrReload ecosystem.config.cjs              # Load/reload all
pm2 startOrReload ecosystem.config.cjs --only llm-gateway  # Specific service
pm2 save                            # Save current process list
pm2 resurrect                       # Restore saved process list

# ─── Canonical Backend Feeds (UI input contracts) ────────────────────────────
node scripts/generate-capability-catalog.mjs   # Regenerates contracts/generated/capability-catalog.v1.json
./scripts/fuel-reconcile-daily.sh              # Runs daily L0 valuation reconcile window
```

### 2.3 Service Names

| PM2 Name | Service | Port | Binary |
|----------|---------|------|--------|
| `llm-gateway` | LLM Gateway | 7700 | Rust |
| `code247` | Autonomous Coder | 4001 | Rust |
| `obs-api` | Dashboard | 3001 | Node.js |
| `cloudflared` | Tunnel | N/A | cloudflared |

### 2.4 Environment Management

View environment for a service:
```bash
pm2 env <pm2-id>                    # Use PM2 ID from `pm2 status`
```

Update environment (requires restart):
```bash
pm2 restart <service> --update-env
```

**Important:** Do NOT put secrets directly in ecosystem.config.cjs. Use:
1. Doppler (`logline-ecosystem`, config `dev/staging/prod`) as central vault
2. `doppler run` in PM2 process commands (already wired in `/Users/ubl-ops/Integration/ecosystem.config.cjs`)
3. macOS Keychain only for operator-local CLI credentials

Doppler operations:
```bash
cd /Users/ubl-ops/Integration
./scripts/doppler-setup.sh --project logline-ecosystem --config dev
./scripts/doppler-secrets-audit.sh --project logline-ecosystem --config dev
```

Reference:
- `/Users/ubl-ops/Integration/SECRETS_DOPPLER_RUNBOOK.md`
- `/Users/ubl-ops/Integration/secrets/doppler-secrets-manifest.tsv`

### 2.5 Startup Configuration

Enable PM2 to start on boot:
```bash
pm2 startup                         # Generate startup script
pm2 save                            # Save current process list
```

Disable startup:
```bash
pm2 unstartup
```

---

## 3) Cloudflare Tunnel Operations

### 3.1 Configuration

Tunnel config location: `~/.cloudflared/config.yml`

```yaml
tunnel: <tunnel-uuid>
credentials-file: /Users/ubl-ops/.cloudflared/<tunnel-uuid>.json

ingress:
  - hostname: llm-gateway.logline.world
    service: http://localhost:7700
  - hostname: obs-api.logline.world
    service: http://localhost:3001
  - hostname: code247.logline.world
    service: http://localhost:4001
  - service: http_status:404
```

### 3.2 Tunnel Commands

```bash
# ─── Status ───────────────────────────────────────────────────────────────────
cloudflared tunnel info logline     # Tunnel details
cloudflared tunnel list             # All tunnels

# ─── Running ──────────────────────────────────────────────────────────────────
cloudflared tunnel run logline      # Manual run (PM2 handles this)

# ─── DNS ──────────────────────────────────────────────────────────────────────
cloudflared tunnel route dns logline <hostname>  # Add DNS route

# ─── Diagnostics ──────────────────────────────────────────────────────────────
cloudflared tunnel cleanup logline  # Remove stale connections
```

### 3.3 Adding New Service to Tunnel

1. Edit `~/.cloudflared/config.yml`:
```yaml
ingress:
  - hostname: new-service.logline.world
    service: http://localhost:PORT
  # ... existing entries ...
  - service: http_status:404  # MUST be last
```

2. Add DNS route:
```bash
cloudflared tunnel route dns logline new-service.logline.world
```

3. Restart tunnel:
```bash
pm2 restart cloudflared
```

4. Verify:
```bash
curl -I https://new-service.logline.world/health
```

### 3.4 Tunnel Troubleshooting

**Symptoms:** 502 Bad Gateway, connection refused

```bash
# Check if tunnel is running
pm2 status cloudflared

# Check tunnel logs
pm2 logs cloudflared --lines 30

# Verify local service is up
curl http://localhost:7700/health

# Restart tunnel
pm2 restart cloudflared
```

**Symptoms:** DNS not resolving

```bash
# Verify DNS route exists
cloudflared tunnel route dns logline <hostname>

# Check Cloudflare dashboard for DNS records
# DNS records should be CNAME to <tunnel-uuid>.cfargotunnel.com
```

---

## 4) Service Lifecycle Procedures

### 4.1 Starting All Services

```bash
cd /Users/ubl-ops/Integration/logic.logline.world
pm2 startOrReload ecosystem.config.cjs
pm2 save
```

### 4.2 Stopping All Services

```bash
pm2 stop all
```

### 4.3 Restarting Single Service

```bash
# Graceful (for Node.js)
pm2 reload obs-api

# Hard restart (for Rust)
pm2 restart llm-gateway
```

### 4.4 Deploying New Version

#### llm-gateway / code247 (Rust):

```bash
# 1. Build new version
cd /Users/ubl-ops/Integration/llm-gateway.logline.world
cargo build --release

# 2. Restart service
pm2 restart llm-gateway

# 3. Verify
pm2 logs llm-gateway --lines 10 --nostream
curl http://localhost:7700/health
```

#### obs-api (Node.js):

```bash
# 1. Install dependencies if needed
cd /Users/ubl-ops/Integration/obs-api.logline.world
npm install

# 2. Build
npm run build

# 3. Reload service
pm2 reload obs-api

# 4. Verify
pm2 logs obs-api --lines 10 --nostream
curl http://localhost:3001/api/health
```

### 4.5 Rolling Back

```bash
# 1. Revert code (git)
cd /Users/ubl-ops/Integration/<service>
git checkout <previous-commit>

# 2. Rebuild if needed
cargo build --release  # or npm run build

# 3. Restart
pm2 restart <service>
```

---

## 5) Health Checks

### 5.1 Quick Health Check Script

```bash
#!/bin/bash
# health-check.sh

echo "=== PM2 Status ==="
pm2 status

echo ""
echo "=== Service Health ==="
echo -n "llm-gateway: "
curl -s http://localhost:7700/health | jq -r '.status // "FAIL"'

echo -n "code247: "
curl -s http://localhost:4001/health | jq -r '.status // "FAIL"'

echo -n "obs-api: "
curl -s http://localhost:3001/api/health | jq -r '.ok // "FAIL"'

echo ""
echo "=== External (via tunnel) ==="
echo -n "llm-gateway.logline.world: "
curl -s -o /dev/null -w "%{http_code}" https://llm-gateway.logline.world/health

echo ""
echo -n "obs-api.logline.world: "
curl -s -o /dev/null -w "%{http_code}" https://obs-api.logline.world/api/health

echo ""
```

### 5.2 Automated Health Monitoring

Add to crontab:
```bash
*/5 * * * * /Users/ubl-ops/scripts/health-check.sh >> /Users/ubl-ops/logs/health.log 2>&1
```

---

## 6) Incident Response

### 6.1 Service Not Starting

```bash
# 1. Check logs for error
pm2 logs <service> --lines 50 --nostream

# 2. Check if port is in use
lsof -i :<port>

# 3. Kill orphan process if needed
kill -9 <pid>

# 4. Restart service
pm2 restart <service>
```

### 6.2 Service Crashing Repeatedly

```bash
# 1. Check restart count
pm2 show <service>  # Look for "restarts" count

# 2. Check detailed logs
pm2 logs <service> --lines 100 --nostream | grep -i error

# 3. Check memory usage
pm2 monit

# 4. If memory issue, consider increasing limits in ecosystem.config.cjs:
#    max_memory_restart: '500M'
```

### 6.3 Tunnel Disconnected

```bash
# 1. Check cloudflared status
pm2 status cloudflared

# 2. Check for authentication issues
pm2 logs cloudflared --lines 20 --nostream

# 3. Re-authenticate if needed (rare)
cloudflared tunnel login

# 4. Restart tunnel
pm2 restart cloudflared
```

### 6.4 High Latency

```bash
# 1. Check service response times
time curl http://localhost:7700/health

# 2. Check Ollama response times
time curl http://localhost:11434/api/tags

# 3. Check Supabase connectivity
time curl https://aypxnwofjtdnmtxastti.supabase.co/rest/v1/ \
  -H "apikey: <anon-key>"

# 4. Check PM2 metrics
pm2 monit  # Watch CPU/Memory
```

### 6.5 Emergency Stop

```bash
# Stop all services immediately
pm2 kill

# Restart PM2 daemon and services
pm2 resurrect
```

---

## 7) Backup and Recovery

### 7.1 PM2 State Backup

```bash
# Save current process list
pm2 save

# Backup location: ~/.pm2/dump.pm2
cp ~/.pm2/dump.pm2 ~/backups/pm2-dump-$(date +%Y%m%d).pm2
```

### 7.2 Cloudflare Credentials Backup

```bash
# Backup tunnel credentials
cp ~/.cloudflared/<tunnel-uuid>.json ~/backups/
cp ~/.cloudflared/config.yml ~/backups/
```

### 7.3 Recovery from Scratch

```bash
# 1. Install PM2
npm install -g pm2

# 2. Install cloudflared
brew install cloudflared

# 3. Restore cloudflared credentials
cp ~/backups/<tunnel-uuid>.json ~/.cloudflared/
cp ~/backups/config.yml ~/.cloudflared/

# 4. Start services
cd /Users/ubl-ops/Integration/logic.logline.world
pm2 startOrReload ecosystem.config.cjs
pm2 save
pm2 startup
```

---

## 8) Maintenance Windows

### 8.1 Pre-Maintenance

```bash
# 1. Notify users (if applicable)

# 2. Save current state
pm2 save

# 3. Note current versions
git -C /Users/ubl-ops/Integration/llm-gateway.logline.world rev-parse HEAD
git -C /Users/ubl-ops/Integration/code247.logline.world rev-parse HEAD
git -C /Users/ubl-ops/Integration/obs-api.logline.world rev-parse HEAD
```

### 8.2 Post-Maintenance

```bash
# 1. Verify all services healthy
./health-check.sh

# 2. Check logs for errors
pm2 logs --lines 20 --nostream

# 3. Save new state
pm2 save
```

---

## 9) Cheat Sheet

```bash
# ─── Most Common Commands ─────────────────────────────────────────────────────

# Status
pm2 status

# Restart gateway
pm2 restart llm-gateway

# View gateway logs
pm2 logs llm-gateway --lines 20 --nostream

# Reload all from config
pm2 startOrReload ecosystem.config.cjs

# Check tunnel
pm2 logs cloudflared --lines 10 --nostream

# Quick health
curl localhost:7700/health && curl localhost:4001/health && curl localhost:3001/api/health
```

---

## References

- `INTEGRATION_BLUEPRINT.md` — Master integration document
- `SERVICE_TOPOLOGY.md` — Network topology
- PM2 docs: https://pm2.keymetrics.io/docs/
- Cloudflare Tunnel docs: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/
