# FUEL SYSTEM SPECIFICATION

**Version:** 2.0.0  
**Date:** 2026-03-05  
**Status:** Active  
**Scope:** `llm-gateway`, `edge-control`, `code247`, `obs-api`, Supabase  
**Supersedes:** FUEL_FLOW_SPEC_v1.md, FUEL_CALCULATION_FOUNDATION_v1.md, FUEL_LEDGER_SPEC.md

---

## Executive Summary

Fuel is the **common currency** of the LogLine ecosystem. It is not just billing—it is:

- **Engineering instrumentation** (observability of real consumption)
- **Economic governance** (cost control, accountability)
- **Health signal / homeostasis** (pressure-based routing and circuit breakers)
- **Proposal generator** (packs → proposals → gates)

This document consolidates all Fuel specifications into a single authoritative source.

---

# PART I — CONCEPTUAL FOUNDATION

## 1) Design Principles

For Fuel to be reliable, it must be:

1. **Backed by measurable signals** (not subjective estimates)
2. **Reproducible** (same inputs → same fuel result)
3. **Segmented** (per provider/model/app/tenant/user/trace)
4. **Reconciled against provider billing** when available
5. **Auditable and versioned** (all derivations carry provenance)

**Core Invariants:**

- **Append-only events**: Raw measurements are never modified
- **Idempotent emission**: Duplicate submissions are rejected
- **Normalized structure**: All apps emit the same event format
- **Centralized pricing**: Apps emit usage; HQ computes cost

---

## 2) The 3-Layer Model (Heart of the System)

Do not collapse everything into one number too early. Fuel operates in three distinct layers:

```
┌─────────────────────────────────────────────────────────────────┐
│                     LAYER C: Control Currency                    │
│         (Fuel Points / Pressure — for decisions/routing)         │
├─────────────────────────────────────────────────────────────────┤
│                     LAYER B: Valuation Ledger                    │
│         (USD estimated/settled, energy, carbon)                  │
├─────────────────────────────────────────────────────────────────┤
│                     LAYER A: Measurement Ledger                  │
│         (Append-only physical facts)                             │
└─────────────────────────────────────────────────────────────────┘
```

### 2.1 Layer A — Measurement Ledger (Physical Facts)

**What it is:** Append-only record of observable measurements.

**Examples of measurements:**

- tokens in/out/cache
- latency (queue_ms, ttft_ms, total_ms, duration_ms)
- retries, fallbacks
- success/error, error_code
- compute_seconds
- energy/carbon (when measurable)

**Invariants:**

- Append-only (immutable once written)
- Idempotent (via idempotency_key)
- No derived values allowed (no USD, no scores)

### 2.2 Layer B — Valuation Ledger (Money/Energy/Carbon)

**What it is:** Transforms measurements into **USD estimated** and **USD settled** (when available), plus optional energy/carbon.

**Invariants:**

- **NOT append-only**: Accepts UPDATE/UPSERT (reality arrives late)
- Everything is versioned (price_card_version, valuation_version)
- Carries provenance and confidence

### 2.3 Layer C — Control Currency (Fuel Points / Pressure)

**What it is:** Deterministic score for control and autonomy decisions:

- **Circuit breaker**: Cut/limit when pressure rises
- **Routing**: Change provider/model when sick
- **Alerting**: When departing from baseline

**Invariants:**

- Never hide components
- Always decompose: base_cost + penalties
- Policy/config versioned

---

## 3) Precision Levels

Every valuation explicitly declares how "real" it is:

| Level | Name | Description |
|-------|------|-------------|
| **L0** | Estimate-only | price card + tokens, no settlement |
| **L1** | Cloud reconciled | settlement API available (`usd_settled`) |
| **L2** | Local metered | local usage + measured energy (GPU/CPU counters) |
| **L3** | Full | L2 + carbon intensity by region/time + baseline comparison |

**Rule:** Any missing input **downgrades** precision level; never silently fabricate certainty.

---

## 4) Source-of-Truth Hierarchy

When multiple sources exist for the same measurement:

1. **Provider settlement APIs** (authoritative for billed cloud spend)
2. **Provider usage counters** in API responses
3. **Local runtime counters** (Ollama, host metrics, GPU/CPU telemetry)
4. **Model-based estimates** (last resort, explicitly marked)

---

# PART II — DATA MODEL

## 5) Layer A — `fuel_events` Table (Append-Only)

### 5.1 Schema

```sql
CREATE TABLE fuel_events (
  -- Identity
  event_id        TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
  idempotency_key TEXT NOT NULL UNIQUE,
  
  -- Scope
  tenant_id       TEXT NOT NULL REFERENCES tenants(tenant_id),
  app_id          TEXT NOT NULL REFERENCES apps(app_id),
  user_id         TEXT NOT NULL REFERENCES users(user_id),
  
  -- Causality
  trace_id        TEXT,
  parent_event_id TEXT REFERENCES fuel_events(event_id),
  event_type      TEXT NOT NULL,
  
  -- Usage
  units           NUMERIC NOT NULL,
  unit_type       TEXT NOT NULL,
  occurred_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  -- Outcome
  outcome         TEXT NOT NULL DEFAULT 'ok', -- ok|fail|blocked|rolled_back
  
  -- Source
  source          TEXT NOT NULL,
  metadata        JSONB DEFAULT '{}',
  
  -- Audit
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_fuel_events_tenant_app ON fuel_events (tenant_id, app_id, occurred_at DESC);
CREATE INDEX idx_fuel_events_user ON fuel_events (user_id, occurred_at DESC);
CREATE INDEX idx_fuel_events_trace ON fuel_events (trace_id);
CREATE INDEX idx_fuel_events_idempotency ON fuel_events (idempotency_key);

-- Immutability enforcement
CREATE TRIGGER no_modify_fuel
  BEFORE UPDATE OR DELETE ON fuel_events
  FOR EACH ROW EXECUTE FUNCTION app.prevent_fuel_modification();

REVOKE UPDATE, DELETE ON fuel_events FROM authenticated, anon, public;
```

### 5.2 Field Definitions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `event_id` | TEXT | Auto | Unique identifier (UUID) |
| `idempotency_key` | TEXT | Yes | Client-provided dedup key |
| `tenant_id` | TEXT | Yes | Billing organization |
| `app_id` | TEXT | Yes | Source application |
| `user_id` | TEXT | Yes | Acting user |
| `trace_id` | TEXT | No | Request trace for causality |
| `parent_event_id` | TEXT | No | Parent event reference |
| `event_type` | TEXT | Yes | Classification from events.registry.json |
| `units` | NUMERIC | Yes | Quantity consumed |
| `unit_type` | TEXT | Yes | Unit classification |
| `occurred_at` | TIMESTAMPTZ | Yes | When usage occurred |
| `outcome` | TEXT | Yes | ok, fail, blocked, rolled_back |
| `source` | TEXT | Yes | Origin subsystem |
| `metadata` | JSONB | Yes | Structured context (schema per event_type) |
| `created_at` | TIMESTAMPTZ | Auto | When record was written |

### 5.3 Unit Types

| Unit Type | Description | Example Source |
|-----------|-------------|----------------|
| `llm_tokens` | LLM token count | llm-gateway |
| `llm_request` | LLM API call count | llm-gateway |
| `code_job` | Autonomous coding job | code247 |
| `storage_bytes` | Storage consumed | storage service |
| `api_call` | General API call | any service |
| `compute_seconds` | Compute time | code247 |

### 5.4 Required Metadata Dimensions per LLM Event

```json
{
  "provider": "openai|anthropic|ollama|...",
  "model": "gpt-4o|claude-sonnet-4-20250514|llama3|...",
  "mode": "completion|streaming|batch",
  "input_tokens": 1234,
  "output_tokens": 567,
  "cached_input_tokens": 0,
  "queue_ms": 50,
  "ttft_ms": 150,
  "latency_ms": 2500,
  "duration_ms": 2600,
  "success": true,
  "error_code": null,
  "retry_count": 0,
  "fallback_used": false,
  "execution_kind": "cloud|local",
  "host": "api.openai.com",
  "region": "us-east-1"
}
```

---

## 6) Layer B — `fuel_valuations` Table (Upsertable)

### 6.1 Schema

```sql
CREATE TABLE fuel_valuations (
  -- Key (1:1 with fuel_events)
  event_id            TEXT PRIMARY KEY REFERENCES fuel_events(event_id),
  
  -- Economic values
  usd_estimated       NUMERIC NOT NULL,
  usd_settled         NUMERIC,  -- nullable until reconciliation
  
  -- Energy/Carbon (optional)
  energy_kwh          NUMERIC,
  carbon_gco2e        NUMERIC,
  
  -- Versioning & Provenance
  price_card_version  TEXT NOT NULL,
  valuation_version   TEXT NOT NULL,
  valuation_source    TEXT NOT NULL, -- price_card|provider_usage_api|provider_cost_api|metered
  precision_level     TEXT NOT NULL, -- L0|L1|L2|L3
  confidence          NUMERIC NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  
  -- Audit
  created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Allow updates for reconciliation
CREATE INDEX idx_fuel_valuations_unsettled ON fuel_valuations (event_id) 
  WHERE usd_settled IS NULL;
```

### 6.2 Field Definitions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `event_id` | TEXT | Yes | FK to fuel_events (1:1) |
| `usd_estimated` | NUMERIC | Yes | Estimated cost from price card |
| `usd_settled` | NUMERIC | No | Actual cost from provider (null until reconciled) |
| `energy_kwh` | NUMERIC | No | Energy consumption |
| `carbon_gco2e` | NUMERIC | No | Carbon footprint |
| `price_card_version` | TEXT | Yes | Version of price card used |
| `valuation_version` | TEXT | Yes | Version of valuation algorithm |
| `valuation_source` | TEXT | Yes | How value was determined |
| `precision_level` | TEXT | Yes | L0, L1, L2, or L3 |
| `confidence` | NUMERIC | Yes | 0.0 to 1.0 |

---

## 7) Layer C — `fuel_points` View (Derived)

```sql
CREATE OR REPLACE VIEW fuel_points AS
SELECT
  e.event_id,
  e.tenant_id,
  e.app_id,
  e.user_id,
  e.trace_id,
  e.occurred_at,
  
  -- Base cost (economic anchor)
  COALESCE(v.usd_settled, v.usd_estimated) * 1000 AS base_cost_points,
  
  -- Penalties (computed from metadata)
  CASE 
    WHEN (e.metadata->>'latency_ms')::numeric > 5000 
    THEN ((e.metadata->>'latency_ms')::numeric - 5000) / 5000 * 100
    ELSE 0 
  END AS penalty_latency,
  
  CASE 
    WHEN e.outcome = 'fail' THEN 100
    WHEN (e.metadata->>'retry_count')::int > 0 THEN (e.metadata->>'retry_count')::int * 20
    WHEN (e.metadata->>'fallback_used')::boolean THEN 50
    ELSE 0
  END AS penalty_errors,
  
  COALESCE(v.energy_kwh * 10, 0) AS penalty_energy,
  
  -- Total
  (COALESCE(v.usd_settled, v.usd_estimated) * 1000) 
    * (1 + 
       CASE WHEN (e.metadata->>'latency_ms')::numeric > 5000 
            THEN ((e.metadata->>'latency_ms')::numeric - 5000) / 5000 * 0.1 ELSE 0 END
       +
       CASE WHEN e.outcome = 'fail' THEN 0.1
            WHEN (e.metadata->>'retry_count')::int > 0 THEN (e.metadata->>'retry_count')::int * 0.02
            WHEN (e.metadata->>'fallback_used')::boolean THEN 0.05
            ELSE 0 END
       +
       COALESCE(v.energy_kwh * 0.01, 0)
    ) AS fuel_points_total,
  
  -- Metadata
  v.precision_level,
  v.confidence,
  'policy-v1' AS policy_version,
  now() AS computed_at

FROM fuel_events e
LEFT JOIN fuel_valuations v ON v.event_id = e.event_id;
```

---

## 8) Pricing Rules Table

```sql
CREATE TABLE pricing_rules (
  rule_id        TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
  version        TEXT NOT NULL,
  unit_type      TEXT NOT NULL,
  source_pattern TEXT,  -- regex to match source (e.g., 'openai:%')
  price_per_unit NUMERIC NOT NULL,
  currency       TEXT NOT NULL DEFAULT 'USD',
  effective_from TIMESTAMPTZ NOT NULL,
  effective_until TIMESTAMPTZ,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  
  UNIQUE (version, unit_type, source_pattern)
);

-- Example pricing rules
INSERT INTO pricing_rules (version, unit_type, source_pattern, price_per_unit, effective_from) VALUES
  ('2026-03', 'llm_tokens', 'openai:%', 0.00003, '2026-03-01'),
  ('2026-03', 'llm_tokens', 'anthropic:%', 0.000024, '2026-03-01'),
  ('2026-03', 'llm_tokens', 'ollama:%', 0.0, '2026-03-01'),  -- Local is free
  ('2026-03', 'code_job', '%', 0.50, '2026-03-01'),
  ('2026-03', 'llm_request', '%', 0.001, '2026-03-01');
```

---

# PART III — CALCULATION ALGORITHMS

## 9) Measurement → Valuation (A → B)

### 9.1 Real-Time Estimation

For each LLM event at emission time:

```
usd_estimated = price(provider, model, input_tokens, output_tokens, cached_tokens, mode)
precision_level = L0
confidence = 0.6–0.9 (depends on counter quality)
```

### 9.2 Batch Reconciliation

A reconciler (cron job):

1. Fetches events where `usd_settled IS NULL` in window (D-1, D-2)
2. Calls provider usage/cost API
3. Fills `usd_settled`
4. Upgrades `precision_level` to L1
5. Sets `confidence = 1.0` (when settlement is authoritative)

**Rule:** Valuation is UPSERT — the world arrives late.

---

## 10) Valuation → Control (B → C) — Formula v1

### 10.1 Economic Anchor (Base)

```
base_cost_points = usd_effective * 1000
usd_effective = usd_settled ?? usd_estimated
```

This anchors Fuel to milli-dollar equivalents.

### 10.2 Pressure Penalties (Dimensionless)

```
penalty_latency = k_lat * max(0, p95_latency_ms - slo_p95_ms) / slo_p95_ms
penalty_errors  = k_err * error_rate_burn
penalty_energy  = k_energy * energy_kwh
```

Where:
- `k_lat`, `k_err`, `k_energy` are configurable multipliers
- `slo_p95_ms` is the SLO for p95 latency (e.g., 5000ms)
- `error_rate_burn` considers error rate over a rolling window

### 10.3 Final Score

```
fuel_points_total = base_cost_points * (1 + penalty_latency + penalty_errors + penalty_energy)
```

**Rules:**

- If data is missing → penalty = 0 but `confidence` drops (visible in dashboard)
- All parameters versioned: `policy_version`, `price_card_version`

---

## 11) Cost Calculation Function

```sql
CREATE OR REPLACE FUNCTION app.calculate_fuel_cost(
  p_tenant_id TEXT,
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_pricing_version TEXT DEFAULT NULL
)
RETURNS TABLE (
  app_id TEXT,
  unit_type TEXT,
  source TEXT,
  total_units NUMERIC,
  price_per_unit NUMERIC,
  total_cost NUMERIC,
  currency TEXT
)
LANGUAGE plpgsql
STABLE
AS $$
BEGIN
  RETURN QUERY
  WITH events AS (
    SELECT 
      fe.app_id,
      fe.unit_type,
      fe.source,
      SUM(fe.units) AS total_units
    FROM fuel_events fe
    WHERE fe.tenant_id = p_tenant_id
      AND fe.occurred_at >= p_start_date
      AND fe.occurred_at < p_end_date
    GROUP BY 1, 2, 3
  ),
  matched_rules AS (
    SELECT DISTINCT ON (e.app_id, e.unit_type, e.source)
      e.*,
      pr.price_per_unit,
      pr.currency
    FROM events e
    LEFT JOIN pricing_rules pr ON
      pr.unit_type = e.unit_type
      AND (pr.source_pattern IS NULL OR e.source LIKE pr.source_pattern)
      AND pr.effective_from <= p_end_date
      AND (pr.effective_until IS NULL OR pr.effective_until > p_start_date)
      AND (p_pricing_version IS NULL OR pr.version = p_pricing_version)
    ORDER BY e.app_id, e.unit_type, e.source, pr.effective_from DESC
  )
  SELECT 
    mr.app_id,
    mr.unit_type,
    mr.source,
    mr.total_units,
    COALESCE(mr.price_per_unit, 0) AS price_per_unit,
    mr.total_units * COALESCE(mr.price_per_unit, 0) AS total_cost,
    COALESCE(mr.currency, 'USD') AS currency
  FROM matched_rules mr
  ORDER BY total_cost DESC;
END;
$$;
```

---

# PART IV — SERVICE INTEGRATION

## 12) Event Emission Protocol

### 12.1 Rust Event Structure

```rust
pub struct FuelEvent {
    pub idempotency_key: String,
    pub tenant_id: String,
    pub app_id: String,
    pub user_id: String,
    pub trace_id: Option<String>,
    pub event_type: String,
    pub units: f64,
    pub unit_type: String,
    pub occurred_at: DateTime<Utc>,
    pub outcome: String,  // "ok", "fail", "blocked", "rolled_back"
    pub source: String,
    pub metadata: serde_json::Value,
}
```

### 12.2 Idempotency Key Generation

The idempotency key MUST be deterministic and unique per billable action.

**Pattern:** `{app_id}:{action_type}:{unique_identifier}:{timestamp_window}`

**Examples:**
```
llm-gateway:completion:req-abc123:2026-03-02T10:00
code247:job:job-xyz789:2026-03-02T10:15
code247:llm-call:job-xyz789:planning:1
```

**Rules:**

- Same idempotency_key = same event (idempotent)
- Retry with same key will not create duplicate
- Different actions MUST have different keys

### 12.3 Emission Timing

| Event Type | When to Emit |
|------------|--------------|
| LLM completion | After successful response |
| LLM stream | After stream completes |
| Code job | After job completes (success or fail) |
| Storage upload | After upload succeeds |

**Rule:** Emit AFTER the billable action completes, not before.

---

## 13) llm-gateway Integration

### 13.1 Responsibilities

- Emit usage per request (tokens, latency, provider/model, cache)
- For local Ollama requests, capture usage fields returned by Ollama
- Periodically reconcile cloud usage/cost with provider admin APIs

### 13.2 Emission Code

```rust
async fn emit_llm_fuel(
    supabase: &SupabaseClient,
    req: &ChatRequest,
    response: &ChatResponse,
    tenant_id: &str,
    user_id: &str,
) -> Result<()> {
    let event = FuelEvent {
        idempotency_key: format!(
            "llm-gateway:completion:{}:{}",
            response.id,
            Utc::now().format("%Y-%m-%dT%H:%M")
        ),
        tenant_id: tenant_id.to_string(),
        app_id: "llm-gateway".to_string(),
        user_id: user_id.to_string(),
        trace_id: req.trace_id.clone(),
        event_type: "llm.completion".to_string(),
        units: response.usage.total_tokens as f64,
        unit_type: "llm_tokens".to_string(),
        occurred_at: Utc::now(),
        outcome: "ok".to_string(),
        source: format!("{}:{}", response.provider, response.model),
        metadata: json!({
            "provider": response.provider,
            "model": response.model,
            "mode": if response.was_streaming { "streaming" } else { "completion" },
            "input_tokens": response.usage.prompt_tokens,
            "output_tokens": response.usage.completion_tokens,
            "cached_input_tokens": response.usage.cached_tokens.unwrap_or(0),
            "latency_ms": response.latency_ms,
            "ttft_ms": response.ttft_ms,
            "success": true,
            "retry_count": response.retries,
            "fallback_used": response.fallback_used,
            "execution_kind": if response.provider == "ollama" { "local" } else { "cloud" },
        }),
    };
    
    supabase.emit_fuel(event).await
}
```

---

## 14) code247 Integration

### 14.1 Responsibilities

- Emit job lifecycle + per-stage execution metrics
- Emit LLM call references (provider/model/mode/job_id) for attribution

### 14.2 Emission Code

```rust
async fn emit_job_fuel(
    supabase: &SupabaseClient,
    job: &Job,
    tenant_id: &str,
    user_id: &str,
) -> Result<()> {
    let event = FuelEvent {
        idempotency_key: format!(
            "code247:job:{}:{}",
            job.id,
            job.updated_at.format("%Y-%m-%dT%H:%M")
        ),
        tenant_id: tenant_id.to_string(),
        app_id: "code247".to_string(),
        user_id: user_id.to_string(),
        trace_id: Some(job.trace_id.clone()),
        event_type: "code247.job".to_string(),
        units: 1.0,
        unit_type: "code_job".to_string(),
        occurred_at: Utc::now(),
        outcome: match job.status.as_str() {
            "completed" => "ok",
            "failed" => "fail",
            _ => "ok",
        }.to_string(),
        source: "code247:pipeline".to_string(),
        metadata: json!({
            "issue_id": job.issue_id,
            "status": job.status,
            "retries": job.retries,
            "stages_completed": job.stages_completed,
            "duration_ms": job.duration_ms,
        }),
    };
    
    supabase.emit_fuel(event).await
}
```

---

## 15) edge-control Integration

### 15.1 Responsibilities

- Emit policy/opinion/gate events with trace links
- Emit orchestration/API call costs as separate unit types

---

## 16) obs-api Integration

### 16.1 Responsibilities

Visualize:

- Estimated vs settled cost drift
- Fuel component breakdown
- Precision-level coverage over time
- Real-time vs statistical views

---

# PART V — VISUALIZATION (UI/UX)

## 17) Toggle Principal: Realtime vs Estatística

| Mode | Purpose | Data Source |
|------|---------|-------------|
| **Realtime** | Urgent problem detection | Layer A + C (measurement + pressure) |
| **Estatística** | Structural problem analysis | Layer B + C (valuation + pressure overlay) |

---

## 18) Realtime View (Always Comparative)

### 18.1 Core Metrics

- **Fuel rate** (units/sec or points/min)
- **Now vs Baseline** (ratio + p25–p75 band)
- **Breakdown:** total tenant → by app → by user

### 18.2 Baseline Calculation

| Phase | Baseline |
|-------|----------|
| First 5 minutes | Average of current window |
| After 5 min | Rolling average (2h/24h) |
| Long-term | Seasonal (same hour/day of week) |

### 18.3 UI Elements

- Line (now) + band (normal range)
- "Top movers" ranked by ratio
- Feed of notables with trace link

---

## 19) Estatística View (Period Pills)

### 19.1 Period Selection

Today | Yesterday | This Month | Custom Range

### 19.2 Metrics

- USD estimated vs settled (drift)
- Precision level coverage (L0..L3)
- Cost by app/user
- Reliability trends (fail/rollback)
- TTFT/latency trends (p95)
- Hotspots

---

## 20) Query Examples

### 20.1 By Tenant (Monthly Summary)

```sql
SELECT 
  date_trunc('month', occurred_at) AS month,
  app_id,
  unit_type,
  SUM(units) AS total_units,
  COUNT(*) AS event_count
FROM fuel_events
WHERE tenant_id = 'tenant-123'
  AND occurred_at >= date_trunc('month', CURRENT_DATE)
GROUP BY 1, 2, 3
ORDER BY 1 DESC, 2, 3;
```

### 20.2 By User (Daily Detail)

```sql
SELECT 
  date_trunc('day', occurred_at) AS day,
  app_id,
  unit_type,
  source,
  SUM(units) AS total_units,
  COUNT(*) AS event_count
FROM fuel_events
WHERE tenant_id = 'tenant-123'
  AND user_id = 'user-456'
  AND occurred_at >= CURRENT_DATE - INTERVAL '7 days'
GROUP BY 1, 2, 3, 4
ORDER BY 1 DESC, total_units DESC;
```

### 20.3 Real-time Dashboard

```sql
SELECT 
  app_id,
  unit_type,
  SUM(units) AS total_units,
  COUNT(*) AS event_count,
  MAX(occurred_at) AS last_event
FROM fuel_events
WHERE tenant_id = 'tenant-123'
  AND occurred_at >= CURRENT_DATE
GROUP BY 1, 2
ORDER BY total_units DESC;
```

---

# PART VI — CONTROL & GOVERNANCE

## 21) Alerts (Deterministic)

Alerts without LLM involvement:

| Alert | Condition |
|-------|-----------|
| Fuel spike | `fuel_points_rate > baseline * X` |
| Error burn | `error_rate_burn > threshold` |
| Rollback surge | `rollback_count > N` |
| Cost anomaly | Cost spike without volume increase |

All alerts generate `fuel_events` with `event_type: alert.*` and can trigger:

- Notification
- SRE/FinOps proposal
- Draft intention

---

## 22) Pressure Interpretation

Fuel Points is not just "cost." It represents:

- **Sick cost** (latency/errors elevating pressure)
- **Fire** in the system

The control plane can use this for:

- Throttling
- Circuit breaker
- Fallback provider selection
- Concurrency reduction
- Incident brief opening
- Mitigation intention proposal

### 22.1 Alarm vs Insight

| Type | Description |
|------|-------------|
| **Alarm (urgent)** | Deterministic rule on rates and baselines |
| **Insight (structural)** | Recurring analysis (snapshots/diffs) + proposals |

---

## 23) Deterministic Packs

At each window, generate:

1. **Snapshot** (facts)
2. **Diff** (deltas from previous)
3. **Packs** (SRE/FinOps/Quality/Docs/Comms)

LLM only embellishes on top of packs and returns typed Proposal with evidence refs.

---

## 24) Governance (Preventing "JSONB Garbage")

### 24.1 events.registry.json (Mandatory)

Located at `/contracts/events.registry.json`:

- Lists valid `event_type` values
- Defines metadata schema per type
- Defines valid `reason_codes`

### 24.2 Emitter Validation (Rust)

- No emission enters if it doesn't validate
- Emitters log `fuel.emit.invalid` (for self-audit)

### 24.3 Versioned Policy/Config

- `journal-config` (per person/team)
- `policy-set` (transistor gates)
- `price-card` versioned

---

## 25) RLS Policies

### 25.1 Insert Policy

```sql
-- Apps can insert fuel events for their own app_id
CREATE POLICY fuel_events_insert_app ON fuel_events
  FOR INSERT WITH CHECK (
    app.is_app_member(tenant_id, app_id)
  );
```

### 25.2 Select Policy

```sql
-- App admins can read their own app's fuel events
CREATE POLICY fuel_events_select_app_admin ON fuel_events
  FOR SELECT USING (
    app.is_app_admin(tenant_id, app_id)
    OR app.is_tenant_admin(tenant_id)
  );
```

---

## 26) Observability: Real-time Feed

```typescript
const channel = supabase
  .channel('fuel:events:tenant-123')
  .on(
    'postgres_changes',
    {
      event: 'INSERT',
      schema: 'public',
      table: 'fuel_events',
      filter: 'tenant_id=eq.tenant-123'
    },
    (payload) => {
      console.log('New fuel event:', payload.new);
      // Update dashboard
    }
  )
  .subscribe();
```

---

# PART VII — IMPLEMENTATION

## 27) Implementation Sequence (v1)

| Step | Description |
|------|-------------|
| 1 | **Emitters confiáveis**: llm-gateway and code247 emitting Layer A correctly |
| 2 | **Valuation**: price card + `fuel_valuations` + reconciler (cloud) |
| 3 | **Points**: view/materialized + thresholds |
| 4 | **Dashboard**: Toggle realtime/estatística + baseline |
| 5 | **Packs e Proposals**: snapshot → packs → proposal (comms + critical first) |
| 6 | **Autonomy by trust**: trust-engine (newsletter becomes auto) |

---

## 28) Hardening Sequence (Technical)

1. Enforce metadata contract for all emitters (required keys)
2. Add `fuel_valuations` table and valuation worker
3. Add price-card versioning + backfill valuation for existing events
4. Add provider reconciliation jobs (OpenAI/Anthropic first)
5. Add local energy collection path (Ollama + host telemetry)
6. Add `fuel_points` derived view/materialization with component breakdown
7. Add dashboards/alerts on drift and confidence

---

## 29) Migration from SQLite (llm-gateway)

Current SQLite schema (`~/.llm-gateway/fuel.db`):

```sql
SELECT day, calls_total, prompt_tokens, completion_tokens
FROM daily_fuel
ORDER BY day DESC;
```

**Migration Steps:**

1. Export daily aggregates from SQLite
2. Insert as historical `fuel_events` with synthetic idempotency keys
3. Point new emissions to Supabase
4. Archive SQLite file

```bash
# Export from SQLite
sqlite3 ~/.llm-gateway/fuel.db ".mode csv" ".headers on" \
  "SELECT * FROM daily_fuel" > fuel_export.csv

# Each row becomes one fuel_event with:
#   idempotency_key: "migration:llm-gateway:{day}"
#   unit_type: "llm_tokens"
#   units: total_tokens
#   source: "migration:daily_aggregate"
```

---

## 30) Definition of Done (DoD)

- [ ] Every relevant event has `trace_id`, `event_type`, `unit_type`, `quantity`, `outcome`
- [ ] `fuel_valuations` exists and supports UPSERT (L0→L1)
- [ ] `fuel_points_total` is calculable and decomposed
- [ ] **Dashboard:**
  - Realtime: rate + baseline + ratio (total/app/user)
  - Estatística: drift estimated vs settled + coverage L0/L1 + trends
- [ ] **Proposals:**
  - comms drafts with evidence refs
  - critical alerts deterministic
- [ ] Config per user: "Dan mode" working

---

# APPENDIX

## A) Fuel Rate Vocabulary

```
fuel_rate_units_per_sec = Σ quantity / Δt  (per unit_type)
fuel_rate_points_per_min = Σ fuel_points_total / Δt
```

**Baselines:**

- Short-term: rolling average
- Long-term: E[rate | hour of day, day of week]

---

## B) Non-Negotiable Invariants

1. `fuel_events` remains **append-only**
2. Valuation is **derived and versioned**, never overwriting raw facts
3. Every derived value carries **provenance and confidence**
4. Any missing input **downgrades precision level**; never silently fabricates certainty

---

## C) Related Documents

- [LLM_START_HERE.md](LLM_START_HERE.md) — Ecosystem overview
- [INTEGRATION_BLUEPRINT.md](INTEGRATION_BLUEPRINT.md) — Overall architecture
- [contracts/events.registry.json](contracts/events.registry.json) — Event type definitions
- [policy/policy-set.v1.1.json](policy/policy-set.v1.1.json) — Policy configuration
