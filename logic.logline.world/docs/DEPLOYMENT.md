# Logic Deployment Guide

This document covers deployment for `logic.logline.world`.

## 1) What Gets Deployed

`logic.logline.world` is a **CLI workspace** — it produces a single binary (`logline`) that runs on operator machines. There is no server deployment.

### Artifacts

| Artifact | Purpose |
|----------|---------|
| `logline` binary | CLI for operators |
| `supabase/migrations/` | Database migrations |
| `schemas/` | JSON schemas for validation |

## 2) Build

```bash
cd /Users/ubl-ops/Integration/logic.logline.world
cargo build --release -p logline-cli
```

Binary location: `target/release/logline`

## 3) Install (Operator Machine)

```bash
# Copy to PATH
cp target/release/logline /usr/local/bin/

# Or symlink
ln -sf $(pwd)/target/release/logline /usr/local/bin/logline
```

## 4) First-Time Setup

```bash
# Initialize config
logline init

# Store Supabase credentials in Keychain
logline secrets set SUPABASE_URL
logline secrets set SUPABASE_ANON_KEY
logline secrets set SUPABASE_SERVICE_KEY

# Verify setup
logline secrets doctor
```

## 5) Database Migrations

Migrations are applied via CLI with required review:

```bash
# Check pending migrations
logline migrate status

# Review changes (generates receipt)
logline migrate review

# Apply (requires receipt from review)
logline migrate apply --env prod
```

Direct `supabase` CLI usage is also supported:

```bash
cd supabase
supabase db push --linked
```

## 6) CI/CD Integration

The CLI can be used in CI for:

```bash
# Pre-flight readiness
logline ready --pipeline prod

# Run integration tests
logline cicd run --pipeline integration-severe

# Publish intentions to Code247
logline harness intentions publish --root ..
```

## 7) Environment Configuration

Config file location: `~/.config/logline/`

```bash
# List config
logline config list

# Set value
logline config set <key> <value>
```

## 8) Doppler Integration

For team environments, use Doppler:

```bash
# Setup Doppler
cd /Users/ubl-ops/Integration
./scripts/doppler-setup.sh --project logline-ecosystem --config dev

# Run with Doppler secrets
doppler run -- logline <command>
```

## 9) Health Check

```bash
# Full system health
logline secrets doctor

# Pipeline readiness
logline ready --pipeline prod
```
