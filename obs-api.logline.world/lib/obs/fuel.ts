import { db } from '@/db';
import { ensureDbSchema } from '@/db/bootstrap';
import { createHash } from 'crypto';
import { SQL, sql } from 'drizzle-orm';
import { z } from 'zod';

const dashboardPresetSchema = z.enum(['today', 'yesterday', 'month', 'custom']);

export const fuelDashboardQuerySchema = z.object({
  realtime_window_minutes: z.coerce.number().int().min(5).max(1440).default(120),
  max_rows: z.coerce.number().int().min(12).max(1000).default(288),
  preset: dashboardPresetSchema.default('today'),
  from: z.string().trim().optional(),
  to: z.string().trim().optional(),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).optional(),
  policy_version: z.string().trim().min(1).max(128).optional(),
  precision_level: z.enum(['L0', 'L1', 'L2', 'L3']).optional(),
}).superRefine((value, ctx) => {
  if (value.preset !== 'custom') return;
  if (!value.from) {
    ctx.addIssue({ code: z.ZodIssueCode.custom, path: ['from'], message: 'from is required when preset=custom' });
  }
  if (!value.to) {
    ctx.addIssue({ code: z.ZodIssueCode.custom, path: ['to'], message: 'to is required when preset=custom' });
  }
});

export type FuelDashboardQuery = z.infer<typeof fuelDashboardQuerySchema>;

export const fuelReconciliationQuerySchema = z.object({
  days: z.coerce.number().int().min(1).max(90).default(14),
  limit: z.coerce.number().int().min(1).max(500).default(120),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).optional(),
  policy_version: z.string().trim().min(1).max(128).optional(),
  precision_level: z.enum(['L0', 'L1', 'L2', 'L3']).optional(),
  source: z.string().trim().min(1).max(128).optional(),
  provider: z.string().trim().min(1).max(128).optional(),
  model: z.string().trim().min(1).max(128).optional(),
});

export type FuelReconciliationQuery = z.infer<typeof fuelReconciliationQuerySchema>;

export const fuelCalibrationQuerySchema = z.object({
  days: z.coerce.number().int().min(1).max(90).default(14),
  limit: z.coerce.number().int().min(1).max(500).default(120),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).optional(),
  policy_version: z.string().trim().min(1).max(128).optional(),
});

export type FuelCalibrationQuery = z.infer<typeof fuelCalibrationQuerySchema>;

export const fuelAlertsQuerySchema = z.object({
  limit: z.coerce.number().int().min(1).max(200).default(50),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).optional(),
  policy_version: z.string().trim().min(1).max(128).optional(),
});

export type FuelAlertsQuery = z.infer<typeof fuelAlertsQuerySchema>;

export const fuelOpsQuerySchema = z.object({
  days: z.coerce.number().int().min(1).max(30).default(7),
  limit: z.coerce.number().int().min(1).max(500).default(120),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).optional(),
  policy_version: z.string().trim().min(1).max(128).optional(),
});

export type FuelOpsQuery = z.infer<typeof fuelOpsQuerySchema>;

type TimeRange = {
  from: Date;
  to: Date;
};

type RealtimeSeriesRow = {
  window_start: string | Date;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  event_count: string | number | null;
  error_count: string | number | null;
  above_p95_count: string | number | null;
  above_p75_count: string | number | null;
  cold_start_count: string | number | null;
};

type RealtimeTopMoverRow = {
  window_start: string | Date;
  tenant_id: string;
  app_id: string;
  source: string;
  policy_version?: string | null;
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  error_rate: string | number | null;
  fuel_points_p75: string | number | null;
  fuel_points_p95: string | number | null;
  fuel_points_ratio_to_p95: string | number | null;
  envelope_status: string;
};

type StatisticsTotalsRow = {
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
  l0_count: string | number | null;
  l1_count: string | number | null;
  l2_count: string | number | null;
  l3_count: string | number | null;
};

type StatisticsByGroupRow = {
  key: string;
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  error_count: string | number | null;
};

type StatisticsProviderModelRow = {
  provider: string | null;
  model: string | null;
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  error_count: string | number | null;
};

type StatisticsByPolicyRow = {
  policy_version: string | null;
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  confidence_avg: string | number | null;
};

type StatisticsByPrecisionRow = {
  precision_level: string | null;
  event_count: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  confidence_avg: string | number | null;
};

type StatisticsModeProviderRow = {
  mode: string | null;
  provider: string | null;
  request_count: string | number | null;
  success_count: string | number | null;
  failure_count: string | number | null;
  fallback_count: string | number | null;
  timeout_count: string | number | null;
  latency_avg_ms: string | number | null;
  latency_p95_ms: string | number | null;
  latency_p99_ms: string | number | null;
  usd_effective_total: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_per_1k_tokens: string | number | null;
  settled_ratio: string | number | null;
};

type StatisticsDailyRow = {
  day: string | Date;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  event_count: string | number | null;
};

type DriftDailyRow = {
  day: string | Date;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_drift_total: string | number | null;
};

type ReconciliationCoverageRow = {
  event_count: string | number | null;
  settled_count: string | number | null;
  l0_count: string | number | null;
  l1_count: string | number | null;
  l2_count: string | number | null;
  l3_count: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
};

type ReconciliationDriftRow = {
  day: string | Date;
  event_count: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
  usd_drift_total: string | number | null;
};

type ReconciliationByGroupRow = {
  key: string;
  event_count: string | number | null;
  settled_count: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
  usd_drift_total: string | number | null;
};

type ReconciliationByProviderModelRow = {
  provider: string | null;
  model: string | null;
  event_count: string | number | null;
  settled_count: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
  usd_drift_total: string | number | null;
};

type ReconciliationByPolicyPrecisionRow = {
  key: string | null;
  event_count: string | number | null;
  settled_count: string | number | null;
  usd_estimated_total: string | number | null;
  usd_settled_total: string | number | null;
  usd_effective_total: string | number | null;
  usd_drift_total: string | number | null;
  confidence_avg: string | number | null;
};

type CalibrationDailyRow = {
  day: string | Date;
  tenant_id: string | null;
  app_id: string | null;
  policy_version: string | null;
  event_count: string | number | null;
  k_latency_current: string | number | null;
  k_errors_current: string | number | null;
  k_energy_current: string | number | null;
  base_cost_points_total: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  penalty_latency_total: string | number | null;
  penalty_errors_total: string | number | null;
  penalty_energy_total: string | number | null;
  penalty_latency_p95: string | number | null;
  penalty_errors_p95: string | number | null;
  penalty_energy_p95: string | number | null;
  latency_excess_ratio_p95: string | number | null;
  error_signal_p95: string | number | null;
  energy_kwh_p95: string | number | null;
  delta_points_if_k_latency_plus_10pct: string | number | null;
  delta_points_if_k_errors_plus_10pct: string | number | null;
  delta_points_if_k_energy_plus_10pct: string | number | null;
  sensitivity_latency_share: string | number | null;
  sensitivity_errors_share: string | number | null;
  sensitivity_energy_share: string | number | null;
};

type FuelAlertCandidateRow = {
  tenant_id: string | null;
  app_id: string | null;
  policy_version: string | null;
  precision_level: string | null;
  alert_code: string;
  severity: string;
  detected_at: string | Date;
  summary: string;
  details: Record<string, unknown> | null;
};

type FuelOpsRunRow = {
  id: string | number;
  job_name: string;
  evidence_date: string | Date;
  tenant_id: string | null;
  app_id: string | null;
  policy_version: string | null;
  status: string;
  started_at: string | Date;
  completed_at: string | Date | null;
  metrics: Record<string, unknown> | null;
  evidence: Record<string, unknown> | null;
};

function toNumber(value: unknown): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

function toIso(value: unknown): string {
  if (value instanceof Date) return value.toISOString();
  const parsed = new Date(String(value));
  return Number.isNaN(parsed.getTime()) ? new Date(0).toISOString() : parsed.toISOString();
}

function startOfUtcDay(now: Date): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate(), 0, 0, 0, 0));
}

function startOfUtcMonth(now: Date): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1, 0, 0, 0, 0));
}

function parseDateOrNull(raw: string | undefined): Date | null {
  if (!raw) return null;
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) return null;
  return date;
}

function resolveStatisticsRange(query: FuelDashboardQuery, now: Date): TimeRange {
  const todayStart = startOfUtcDay(now);

  if (query.preset === 'today') {
    return { from: todayStart, to: now };
  }

  if (query.preset === 'yesterday') {
    const yesterdayStart = new Date(todayStart.getTime() - 24 * 60 * 60 * 1000);
    return { from: yesterdayStart, to: todayStart };
  }

  if (query.preset === 'month') {
    return { from: startOfUtcMonth(now), to: now };
  }

  const customFrom = parseDateOrNull(query.from);
  const customTo = parseDateOrNull(query.to);
  if (!customFrom || !customTo || customFrom >= customTo) {
    return { from: todayStart, to: now };
  }
  return { from: customFrom, to: customTo };
}

function isUndefinedRelationError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false;
  const candidate = error as { code?: string };
  return candidate.code === '42P01';
}

async function executeRows<T extends Record<string, unknown>>(query: SQL): Promise<T[]> {
  const result = await db.execute(query) as unknown;
  if (Array.isArray(result)) return result as T[];
  if (result && typeof result === 'object' && 'rows' in result) {
    const rows = (result as { rows?: unknown }).rows;
    if (Array.isArray(rows)) return rows as T[];
  }
  return [];
}

function toFuelAlertId(code: string, tenantId: string | null, appId: string | null, policyVersion: string | null, precisionLevel: string | null): string {
  const digest = createHash('sha1')
    .update([code, tenantId ?? '-', appId ?? '-', policyVersion ?? '-', precisionLevel ?? '-'].join(':'))
    .digest('hex');
  return `fuel_${digest.slice(0, 24)}`;
}

async function loadFuelAlertCandidates(filter: {
  tenant_id?: string;
  app_id?: string;
  policy_version?: string;
} = {}): Promise<FuelAlertCandidateRow[]> {
  const tenantId = filter.tenant_id ?? null;
  const appId = filter.app_id ?? null;
  const policyVersion = filter.policy_version ?? null;

  return executeRows<FuelAlertCandidateRow>(sql`
    select
      tenant_id,
      app_id,
      policy_version,
      precision_level,
      alert_code,
      severity,
      detected_at,
      summary,
      details
    from fuel_alert_candidates_v1
    where (${tenantId}::text is null or tenant_id = ${tenantId})
      and (${appId}::text is null or app_id = ${appId})
      and (${policyVersion}::text is null or policy_version = ${policyVersion})
    order by detected_at desc, alert_code asc
  `);
}

export async function syncFuelAlerts(filter: {
  tenant_id?: string;
  app_id?: string;
  policy_version?: string;
  limit?: number;
} = {}) {
  await ensureDbSchema();

  const now = new Date();
  const nowIso = now.toISOString();
  const rows = await loadFuelAlertCandidates(filter);
  const alertIds = rows.map((row) => toFuelAlertId(row.alert_code, row.tenant_id, row.app_id, row.policy_version, row.precision_level));
  const staleAlertClause = alertIds.length > 0
    ? sql`and alert_id not in (${sql.join(alertIds.map((value) => sql`${value}`), sql`, `)})`
    : sql``;

  for (const row of rows) {
    const alertId = toFuelAlertId(row.alert_code, row.tenant_id, row.app_id, row.policy_version, row.precision_level);
    await db.execute(sql`
      insert into obs_alerts (
        alert_id,
        code,
        severity,
        status,
        summary,
        details,
        source,
        first_seen_at,
        last_seen_at,
        created_at,
        updated_at
      ) values (
        ${alertId},
        ${row.alert_code},
        ${row.severity},
        'open',
        ${row.summary},
        ${JSON.stringify({
          ...(row.details ?? {}),
          tenant_id: row.tenant_id,
          app_id: row.app_id,
          policy_version: row.policy_version,
          precision_level: row.precision_level,
          detected_at: toIso(row.detected_at),
        })}::jsonb,
        'fuel',
        cast(${nowIso} as timestamptz),
        cast(${nowIso} as timestamptz),
        cast(${nowIso} as timestamptz),
        cast(${nowIso} as timestamptz)
      )
      on conflict (alert_id) do update
        set code = excluded.code,
            severity = excluded.severity,
            status = case when obs_alerts.status = 'resolved' then 'open' else obs_alerts.status end,
            summary = excluded.summary,
            details = excluded.details,
            source = excluded.source,
            last_seen_at = excluded.last_seen_at,
            resolved_at = null,
            updated_at = excluded.updated_at
    `);
  }

  await db.execute(sql`
    update obs_alerts
    set status = 'resolved',
        resolved_at = cast(${nowIso} as timestamptz),
        updated_at = cast(${nowIso} as timestamptz)
    where source = 'fuel'
      and code like 'fuel.%'
      and (${filter.tenant_id ?? null}::text is null or details->>'tenant_id' = ${filter.tenant_id ?? null})
      and (${filter.app_id ?? null}::text is null or details->>'app_id' = ${filter.app_id ?? null})
      and (${filter.policy_version ?? null}::text is null or details->>'policy_version' = ${filter.policy_version ?? null})
      ${staleAlertClause}
      and status in ('open', 'acked')
  `);

  const openRows = await executeRows<Record<string, unknown>>(sql`
    select
      alert_id,
      code,
      severity,
      status,
      summary,
      details,
      source,
      first_seen_at,
      last_seen_at,
      acked_at,
      acked_by,
      ack_reason,
      resolved_at,
      created_at,
      updated_at
    from obs_alerts
    where source = 'fuel'
      and status = 'open'
      and (${filter.tenant_id ?? null}::text is null or details->>'tenant_id' = ${filter.tenant_id ?? null})
      and (${filter.app_id ?? null}::text is null or details->>'app_id' = ${filter.app_id ?? null})
      and (${filter.policy_version ?? null}::text is null or details->>'policy_version' = ${filter.policy_version ?? null})
    order by last_seen_at desc
    limit ${filter.limit ?? 50}
  `);

  const countsRows = await executeRows<Record<string, unknown>>(sql`
    select
      count(*) filter (where status = 'open')::bigint as open_count,
      count(*) filter (where status = 'acked')::bigint as acked_count,
      count(*) filter (where status = 'resolved')::bigint as resolved_count
    from obs_alerts
    where source = 'fuel'
      and code like 'fuel.%'
      and (${filter.tenant_id ?? null}::text is null or details->>'tenant_id' = ${filter.tenant_id ?? null})
      and (${filter.app_id ?? null}::text is null or details->>'app_id' = ${filter.app_id ?? null})
      and (${filter.policy_version ?? null}::text is null or details->>'policy_version' = ${filter.policy_version ?? null})
  `);

  const counts = countsRows[0] ?? {};
  return {
    generated_at: now.toISOString(),
    open_count: toNumber(counts.open_count),
    acked_count: toNumber(counts.acked_count),
    resolved_count: toNumber(counts.resolved_count),
    items: openRows.map((row) => ({
      alert_id: String(row.alert_id ?? ''),
      code: String(row.code ?? ''),
      severity: String(row.severity ?? 'warn'),
      status: String(row.status ?? 'open'),
      summary: String(row.summary ?? ''),
      details: row.details ?? {},
      source: String(row.source ?? 'fuel'),
      first_seen_at: toIso(row.first_seen_at),
      last_seen_at: toIso(row.last_seen_at),
      acked_at: row.acked_at ? toIso(row.acked_at) : null,
      acked_by: row.acked_by ?? null,
      ack_reason: row.ack_reason ?? null,
      resolved_at: row.resolved_at ? toIso(row.resolved_at) : null,
      created_at: toIso(row.created_at),
      updated_at: toIso(row.updated_at),
    })),
  };
}

async function queryRealtimeWithBaseline(
  tenantId: string | null,
  appId: string | null,
  policyVersion: string | null,
  windowMinutes: number,
  maxRows: number,
): Promise<{ series: RealtimeSeriesRow[]; movers: RealtimeTopMoverRow[]; baselineReady: boolean }> {
  const rows = await executeRows<RealtimeSeriesRow>(sql`
    with filtered as (
      select *
      from fuel_window_realtime_v1
      where window_start >= now() - (${windowMinutes} * interval '1 minute')
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
    )
    select
      window_start,
      sum(fuel_points_total)::numeric as fuel_points_total,
      sum(usd_effective_total)::numeric as usd_effective_total,
      sum(event_count)::bigint as event_count,
      sum(error_count)::bigint as error_count,
      sum(case when envelope_status = 'above_p95' then event_count else 0 end)::bigint as above_p95_count,
      sum(case when envelope_status = 'above_p75' then event_count else 0 end)::bigint as above_p75_count,
      sum(case when envelope_status = 'cold_start' then event_count else 0 end)::bigint as cold_start_count
    from filtered
    group by window_start
    order by window_start desc
    limit ${maxRows}
  `);

  const movers = await executeRows<RealtimeTopMoverRow>(sql`
    with filtered as (
      select *
      from fuel_window_realtime_v1
      where window_start >= now() - (${windowMinutes} * interval '1 minute')
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
    ), latest as (
      select max(window_start) as window_start from filtered
    )
    select
      f.window_start,
      f.tenant_id,
      f.app_id,
      f.source,
      f.policy_version,
      f.event_count,
      f.fuel_points_total,
      f.usd_effective_total,
      f.error_rate,
      f.fuel_points_p75,
      f.fuel_points_p95,
      f.fuel_points_ratio_to_p95,
      f.envelope_status
    from filtered f
    join latest l on l.window_start = f.window_start
    order by coalesce(f.fuel_points_ratio_to_p95, 0) desc, f.fuel_points_total desc
    limit 10
  `);

  return {
    series: rows,
    movers,
    baselineReady: movers.some((row) => row.envelope_status !== 'cold_start'),
  };
}

async function queryRealtimeWithoutBaseline(
  tenantId: string | null,
  appId: string | null,
  policyVersion: string | null,
  precisionLevel: string | null,
  windowMinutes: number,
  maxRows: number,
): Promise<{ series: RealtimeSeriesRow[]; movers: RealtimeTopMoverRow[]; baselineReady: boolean }> {
  const rows = await executeRows<RealtimeSeriesRow>(sql`
    with binned as (
      select
        to_timestamp(floor(extract(epoch from occurred_at) / 300) * 300)::timestamptz as window_start,
        app_id,
        source,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(case when coalesce(outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count
      from fuel_points_v1
      where occurred_at >= now() - (${windowMinutes} * interval '1 minute')
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by 1, 2, 3
    )
    select
      window_start,
      sum(fuel_points_total)::numeric as fuel_points_total,
      sum(usd_effective_total)::numeric as usd_effective_total,
      sum(event_count)::bigint as event_count,
      sum(error_count)::bigint as error_count,
      0::bigint as above_p95_count,
      0::bigint as above_p75_count,
      sum(event_count)::bigint as cold_start_count
    from binned
    group by window_start
    order by window_start desc
    limit ${maxRows}
  `);

  const movers = await executeRows<RealtimeTopMoverRow>(sql`
    with binned as (
      select
        to_timestamp(floor(extract(epoch from occurred_at) / 300) * 300)::timestamptz as window_start,
        tenant_id,
        app_id,
        source,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        case when count(*) > 0
          then (sum(case when coalesce(outcome, 'ok') <> 'ok' then 1 else 0 end)::numeric / count(*)::numeric)
          else 0::numeric
        end as error_rate
      from fuel_points_v1
      where occurred_at >= now() - (${windowMinutes} * interval '1 minute')
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by 1, 2, 3, 4
    ), latest as (
      select max(window_start) as window_start from binned
    )
    select
      b.window_start,
      b.tenant_id,
      b.app_id,
      b.source,
      null::text as policy_version,
      b.event_count,
      b.fuel_points_total,
      b.usd_effective_total,
      b.error_rate,
      null::numeric as fuel_points_p75,
      null::numeric as fuel_points_p95,
      null::numeric as fuel_points_ratio_to_p95,
      'cold_start'::text as envelope_status
    from binned b
    join latest l on l.window_start = b.window_start
    order by b.fuel_points_total desc
    limit 10
  `);

  return { series: rows, movers, baselineReady: false };
}

export async function getFuelDashboard(query: FuelDashboardQuery) {
  await ensureDbSchema();

  const now = new Date();
  const statisticsRange = resolveStatisticsRange(query, now);
  const statisticsFrom = statisticsRange.from.toISOString();
  const statisticsTo = statisticsRange.to.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? null;
  const policyVersion = query.policy_version ?? null;
  const precisionLevel = query.precision_level ?? null;

  let realtimePayload: { series: RealtimeSeriesRow[]; movers: RealtimeTopMoverRow[]; baselineReady: boolean };
  const canUseBaseline = !precisionLevel;
  try {
    realtimePayload = canUseBaseline
      ? await queryRealtimeWithBaseline(tenantId, appId, policyVersion, query.realtime_window_minutes, query.max_rows)
      : await queryRealtimeWithoutBaseline(tenantId, appId, policyVersion, precisionLevel, query.realtime_window_minutes, query.max_rows);
  } catch (error) {
    if (!isUndefinedRelationError(error)) throw error;
    realtimePayload = await queryRealtimeWithoutBaseline(
      tenantId,
      appId,
      policyVersion,
      precisionLevel,
      query.realtime_window_minutes,
      query.max_rows,
    );
  }

  const [totalsRows, byAppRows, bySourceRows, providerModelRows, dailyRows, driftRows, byPolicyRows, byPrecisionRows, modeProviderRows, alerts] = await Promise.all([
    executeRows<StatisticsTotalsRow>(sql`
      select
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(case when precision_level = 'L0' then 1 else 0 end)::bigint as l0_count,
        sum(case when precision_level = 'L1' then 1 else 0 end)::bigint as l1_count,
        sum(case when precision_level = 'L2' then 1 else 0 end)::bigint as l2_count,
        sum(case when precision_level = 'L3' then 1 else 0 end)::bigint as l3_count
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
    `),
    executeRows<StatisticsByGroupRow>(sql`
      select
        app_id as key,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(case when coalesce(outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by app_id
      order by fuel_points_total desc
      limit 20
    `),
    executeRows<StatisticsByGroupRow>(sql`
      select
        source as key,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(case when coalesce(outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by source
      order by fuel_points_total desc
      limit 20
    `),
    executeRows<StatisticsProviderModelRow>(sql`
      select
        coalesce(fe.metadata->>'provider', 'unknown') as provider,
        coalesce(fe.metadata->>'model', 'unknown') as model,
        count(*)::bigint as event_count,
        sum(coalesce(fp.fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
        sum(case when coalesce(fp.outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count
      from fuel_events fe
      left join fuel_points_v1 fp on fp.event_id = fe.event_id
      where fe.occurred_at >= cast(${statisticsFrom} as timestamptz)
        and fe.occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${policyVersion}::text is null or fp.policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or fp.precision_level = ${precisionLevel})
      group by coalesce(fe.metadata->>'provider', 'unknown'), coalesce(fe.metadata->>'model', 'unknown')
      order by fuel_points_total desc
      limit 40
    `),
    executeRows<StatisticsDailyRow>(sql`
      select
        date_trunc('day', occurred_at) as day,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        count(*)::bigint as event_count
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by date_trunc('day', occurred_at)
      order by day asc
    `),
    executeRows<DriftDailyRow>(sql`
      select
        date_trunc('day', day) as day,
        sum(usd_estimated_total)::numeric as usd_estimated_total,
        sum(usd_settled_total)::numeric as usd_settled_total,
        sum(usd_drift_total)::numeric as usd_drift_total
      from fuel_valuation_drift_v1
      where day >= cast(${statisticsFrom} as timestamptz)
        and day < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
      group by date_trunc('day', day)
      order by day asc
    `),
    executeRows<StatisticsByPolicyRow>(sql`
      select
        policy_version,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        avg(coalesce(confidence, 0))::numeric as confidence_avg
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by policy_version
      order by fuel_points_total desc
    `),
    executeRows<StatisticsByPrecisionRow>(sql`
      select
        precision_level,
        count(*)::bigint as event_count,
        sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        avg(coalesce(confidence, 0))::numeric as confidence_avg
      from fuel_points_v1
      where occurred_at >= cast(${statisticsFrom} as timestamptz)
        and occurred_at < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by precision_level
      order by fuel_points_total desc
    `),
    executeRows<StatisticsModeProviderRow>(sql`
      select
        mode,
        provider,
        sum(request_count)::bigint as request_count,
        sum(success_count)::bigint as success_count,
        sum(failure_count)::bigint as failure_count,
        sum(fallback_count)::bigint as fallback_count,
        sum(timeout_count)::bigint as timeout_count,
        avg(latency_avg_ms)::numeric as latency_avg_ms,
        percentile_cont(0.95) within group (order by coalesce(latency_p95_ms, 0))::numeric as latency_p95_ms,
        percentile_cont(0.99) within group (order by coalesce(latency_p99_ms, 0))::numeric as latency_p99_ms,
        sum(usd_effective_total)::numeric as usd_effective_total,
        sum(fuel_points_total)::numeric as fuel_points_total,
        avg(usd_effective_per_1k_tokens)::numeric as usd_effective_per_1k_tokens,
        avg(settled_ratio)::numeric as settled_ratio
      from fuel_llm_mode_provider_metrics_v1
      where day >= cast(${statisticsFrom} as timestamptz)
        and day < cast(${statisticsTo} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
      group by mode, provider
      order by usd_effective_total desc, request_count desc
    `),
    syncFuelAlerts({
      tenant_id: query.tenant_id,
      app_id: query.app_id,
      policy_version: query.policy_version,
      limit: 10,
    }),
  ]);

  const series = [...realtimePayload.series]
    .reverse()
    .map((row) => ({
      window_start: toIso(row.window_start),
      fuel_points_total: toNumber(row.fuel_points_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      event_count: toNumber(row.event_count),
      error_count: toNumber(row.error_count),
      above_p95_count: toNumber(row.above_p95_count),
      above_p75_count: toNumber(row.above_p75_count),
      cold_start_count: toNumber(row.cold_start_count),
    }));

  const latestSeries = series.length > 0 ? series[series.length - 1] : null;
  const pressure = (() => {
    if (!latestSeries) {
      return {
        critical: 0,
        warn: 0,
        ok: 0,
        cold_start: 0,
        coverage_ratio: 0,
      };
    }

    const critical = latestSeries.above_p95_count;
    const warn = latestSeries.above_p75_count;
    const coldStart = latestSeries.cold_start_count;
    const ok = Math.max(latestSeries.event_count - critical - warn - coldStart, 0);
    const coverageRatio = latestSeries.event_count > 0
      ? (critical + warn + ok) / latestSeries.event_count
      : 0;

    return {
      critical,
      warn,
      ok,
      cold_start: coldStart,
      coverage_ratio: coverageRatio,
    };
  })();

  const totals = totalsRows[0] ?? {
    event_count: 0,
    fuel_points_total: 0,
    usd_estimated_total: 0,
    usd_settled_total: 0,
    usd_effective_total: 0,
    l0_count: 0,
    l1_count: 0,
    l2_count: 0,
    l3_count: 0,
  };

  return {
    generated_at: now.toISOString(),
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      policy_version: policyVersion,
      precision_level: precisionLevel,
    },
    realtime: {
      window: {
        minutes: query.realtime_window_minutes,
        bucket_minutes: 5,
      },
      baseline: {
        status: realtimePayload.baselineReady ? 'ready' : 'cold_start',
        envelope: 'p75/p95',
      },
      current: latestSeries,
      pressure,
      series,
      top_movers: realtimePayload.movers.map((row) => ({
        window_start: toIso(row.window_start),
        tenant_id: row.tenant_id,
        app_id: row.app_id,
        source: row.source,
        policy_version: row.policy_version ?? policyVersion ?? 'unknown',
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        error_rate: toNumber(row.error_rate),
        envelope: {
          p75: toNumber(row.fuel_points_p75),
          p95: toNumber(row.fuel_points_p95),
          ratio_to_p95: toNumber(row.fuel_points_ratio_to_p95),
          status: row.envelope_status,
        },
      })),
    },
    statistics: {
      period: {
        preset: query.preset,
        from: statisticsRange.from.toISOString(),
        to: statisticsRange.to.toISOString(),
      },
      totals: {
        event_count: toNumber(totals.event_count),
        fuel_points_total: toNumber(totals.fuel_points_total),
        usd_estimated_total: toNumber(totals.usd_estimated_total),
        usd_settled_total: toNumber(totals.usd_settled_total),
        usd_effective_total: toNumber(totals.usd_effective_total),
      },
      precision_coverage: {
        L0: toNumber(totals.l0_count),
        L1: toNumber(totals.l1_count),
        L2: toNumber(totals.l2_count),
        L3: toNumber(totals.l3_count),
      },
      alerts: {
        open_count: alerts.open_count,
        acked_count: alerts.acked_count,
        resolved_count: alerts.resolved_count,
        top_open: alerts.items,
      },
      by_app: byAppRows.map((row) => ({
        app_id: row.key,
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        error_count: toNumber(row.error_count),
      })),
      by_source: bySourceRows.map((row) => ({
        source: row.key,
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        error_count: toNumber(row.error_count),
      })),
      by_provider_model: providerModelRows.map((row) => ({
        provider: row.provider ?? 'unknown',
        model: row.model ?? 'unknown',
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        error_count: toNumber(row.error_count),
      })),
      by_policy_version: byPolicyRows.map((row) => ({
        policy_version: row.policy_version ?? 'unassigned',
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        confidence_avg: toNumber(row.confidence_avg),
      })),
      by_precision_level: byPrecisionRows.map((row) => ({
        precision_level: row.precision_level ?? 'L0',
        event_count: toNumber(row.event_count),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        confidence_avg: toNumber(row.confidence_avg),
      })),
      by_mode_provider: modeProviderRows.map((row) => ({
        mode: row.mode ?? 'unknown',
        provider: row.provider ?? 'unknown',
        request_count: toNumber(row.request_count),
        success_count: toNumber(row.success_count),
        failure_count: toNumber(row.failure_count),
        fallback_count: toNumber(row.fallback_count),
        timeout_count: toNumber(row.timeout_count),
        latency_avg_ms: toNumber(row.latency_avg_ms),
        latency_p95_ms: toNumber(row.latency_p95_ms),
        latency_p99_ms: toNumber(row.latency_p99_ms),
        usd_effective_total: toNumber(row.usd_effective_total),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_per_1k_tokens: toNumber(row.usd_effective_per_1k_tokens),
        settled_ratio: toNumber(row.settled_ratio),
      })),
      daily: dailyRows.map((row) => ({
        day: toIso(row.day),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        event_count: toNumber(row.event_count),
      })),
      drift_daily: driftRows.map((row) => ({
        day: toIso(row.day),
        usd_estimated_total: toNumber(row.usd_estimated_total),
        usd_settled_total: toNumber(row.usd_settled_total),
        usd_drift_total: toNumber(row.usd_drift_total),
      })),
    },
  };
}

export async function getFuelReconciliation(query: FuelReconciliationQuery) {
  await ensureDbSchema();

  const now = new Date();
  const from = new Date(now.getTime() - query.days * 24 * 60 * 60 * 1000);
  const fromBoundaryIso = from.toISOString();
  const toBoundaryIso = now.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? null;
  const policyVersion = query.policy_version ?? null;
  const precisionLevel = query.precision_level ?? null;
  const source = query.source ?? null;
  const provider = query.provider ?? null;
  const model = query.model ?? null;

  const [coverageRows, driftRows, byAppRows, bySourceRows, byProviderModelRows, byPolicyRows, byPrecisionRows, alerts] = await Promise.all([
    executeRows<ReconciliationCoverageRow>(sql`
      select
        count(*)::bigint as event_count,
        sum(case when fp.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(case when fp.precision_level = 'L0' then 1 else 0 end)::bigint as l0_count,
        sum(case when fp.precision_level = 'L1' then 1 else 0 end)::bigint as l1_count,
        sum(case when fp.precision_level = 'L2' then 1 else 0 end)::bigint as l2_count,
        sum(case when fp.precision_level = 'L3' then 1 else 0 end)::bigint as l3_count,
        sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total
      from fuel_events fe
      left join fuel_points_v1 fp on fp.event_id = fe.event_id
      where fe.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fe.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${policyVersion}::text is null or fp.policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or fp.precision_level = ${precisionLevel})
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
    `),
    executeRows<ReconciliationDriftRow>(sql`
      select
        date_trunc('day', day) as day,
        sum(event_count)::bigint as event_count,
        sum(usd_estimated_total)::numeric as usd_estimated_total,
        sum(usd_settled_total)::numeric as usd_settled_total,
        sum(usd_effective_total)::numeric as usd_effective_total,
        sum(usd_drift_total)::numeric as usd_drift_total
      from fuel_valuation_drift_v1
      where day >= cast(${fromBoundaryIso} as timestamptz)
        and day < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
        and (${source}::text is null or source = ${source})
      group by date_trunc('day', day)
      order by day desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByGroupRow>(sql`
      select
        fp.app_id as key,
        count(*)::bigint as event_count,
        sum(case when fp.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
        sum(coalesce(fp.usd_settled, 0) - coalesce(fp.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_points_v1 fp
      where fp.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fp.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fp.tenant_id = ${tenantId})
        and (${appId}::text is null or fp.app_id = ${appId})
        and (${policyVersion}::text is null or fp.policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or fp.precision_level = ${precisionLevel})
        and (${source}::text is null or fp.source = ${source})
        and (${provider}::text is null or fp.provider = ${provider})
        and (${model}::text is null or fp.model = ${model})
      group by fp.app_id
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByGroupRow>(sql`
      select
        fp.source as key,
        count(*)::bigint as event_count,
        sum(case when fp.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
        sum(coalesce(fp.usd_settled, 0) - coalesce(fp.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_points_v1 fp
      where fp.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fp.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fp.tenant_id = ${tenantId})
        and (${appId}::text is null or fp.app_id = ${appId})
        and (${policyVersion}::text is null or fp.policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or fp.precision_level = ${precisionLevel})
        and (${source}::text is null or fp.source = ${source})
        and (${provider}::text is null or fp.provider = ${provider})
        and (${model}::text is null or fp.model = ${model})
      group by fp.source
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByProviderModelRow>(sql`
      select
        coalesce(fp.provider, 'unknown') as provider,
        coalesce(fp.model, 'unknown') as model,
        count(*)::bigint as event_count,
        sum(case when fp.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
        sum(coalesce(fp.usd_settled, 0) - coalesce(fp.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_points_v1 fp
      where fp.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fp.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fp.tenant_id = ${tenantId})
        and (${appId}::text is null or fp.app_id = ${appId})
        and (${policyVersion}::text is null or fp.policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or fp.precision_level = ${precisionLevel})
        and (${source}::text is null or fp.source = ${source})
        and (${provider}::text is null or fp.provider = ${provider})
        and (${model}::text is null or fp.model = ${model})
      group by coalesce(fp.provider, 'unknown'), coalesce(fp.model, 'unknown')
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByPolicyPrecisionRow>(sql`
      select
        policy_version as key,
        count(*)::bigint as event_count,
        sum(case when usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(coalesce(usd_settled, 0) - coalesce(usd_estimated, 0))::numeric as usd_drift_total,
        avg(coalesce(confidence, 0))::numeric as confidence_avg
      from fuel_points_v1
      where occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
        and (${source}::text is null or source = ${source})
        and (${provider}::text is null or provider = ${provider})
        and (${model}::text is null or model = ${model})
      group by policy_version
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByPolicyPrecisionRow>(sql`
      select
        precision_level as key,
        count(*)::bigint as event_count,
        sum(case when usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
        sum(coalesce(usd_settled, 0) - coalesce(usd_estimated, 0))::numeric as usd_drift_total,
        avg(coalesce(confidence, 0))::numeric as confidence_avg
      from fuel_points_v1
      where occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or tenant_id = ${tenantId})
        and (${appId}::text is null or app_id = ${appId})
        and (${policyVersion}::text is null or policy_version = ${policyVersion})
        and (${precisionLevel}::text is null or precision_level = ${precisionLevel})
        and (${source}::text is null or source = ${source})
        and (${provider}::text is null or provider = ${provider})
        and (${model}::text is null or model = ${model})
      group by precision_level
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    syncFuelAlerts({
      tenant_id: query.tenant_id,
      app_id: query.app_id,
      policy_version: query.policy_version,
      limit: 10,
    }),
  ]);

  const coverage = coverageRows[0] ?? {
    event_count: 0,
    settled_count: 0,
    l0_count: 0,
    l1_count: 0,
    l2_count: 0,
    l3_count: 0,
    usd_estimated_total: 0,
    usd_settled_total: 0,
    usd_effective_total: 0,
  };

  const totalEvents = toNumber(coverage.event_count);
  const settledEvents = toNumber(coverage.settled_count);

  return {
    generated_at: now.toISOString(),
    window: {
      days: query.days,
      from: fromBoundaryIso,
      to: toBoundaryIso,
    },
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      policy_version: policyVersion,
      precision_level: precisionLevel,
      source,
      provider,
      model,
    },
    coverage: {
      event_count: totalEvents,
      settled_count: settledEvents,
      settled_ratio: totalEvents > 0 ? settledEvents / totalEvents : 0,
      precision_counts: {
        L0: toNumber(coverage.l0_count),
        L1: toNumber(coverage.l1_count),
        L2: toNumber(coverage.l2_count),
        L3: toNumber(coverage.l3_count),
      },
      usd_estimated_total: toNumber(coverage.usd_estimated_total),
      usd_settled_total: toNumber(coverage.usd_settled_total),
      usd_effective_total: toNumber(coverage.usd_effective_total),
      usd_drift_total: toNumber(coverage.usd_settled_total) - toNumber(coverage.usd_estimated_total),
    },
    alerts: {
      open_count: alerts.open_count,
      acked_count: alerts.acked_count,
      resolved_count: alerts.resolved_count,
      top_open: alerts.items,
    },
    drift_daily: driftRows.map((row) => ({
      day: toIso(row.day),
      event_count: toNumber(row.event_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
    })),
    by_app: byAppRows.map((row) => ({
      app_id: row.key,
      event_count: toNumber(row.event_count),
      settled_count: toNumber(row.settled_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
    })),
    by_source: bySourceRows.map((row) => ({
      source: row.key,
      event_count: toNumber(row.event_count),
      settled_count: toNumber(row.settled_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
    })),
    by_provider_model: byProviderModelRows.map((row) => ({
      provider: row.provider ?? 'unknown',
      model: row.model ?? 'unknown',
      event_count: toNumber(row.event_count),
      settled_count: toNumber(row.settled_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
    })),
    by_policy_version: byPolicyRows.map((row) => ({
      policy_version: row.key ?? 'unassigned',
      event_count: toNumber(row.event_count),
      settled_count: toNumber(row.settled_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
      confidence_avg: toNumber(row.confidence_avg),
    })),
    by_precision_level: byPrecisionRows.map((row) => ({
      precision_level: row.key ?? 'L0',
      event_count: toNumber(row.event_count),
      settled_count: toNumber(row.settled_count),
      usd_estimated_total: toNumber(row.usd_estimated_total),
      usd_settled_total: toNumber(row.usd_settled_total),
      usd_effective_total: toNumber(row.usd_effective_total),
      usd_drift_total: toNumber(row.usd_drift_total),
      confidence_avg: toNumber(row.confidence_avg),
    })),
  };
}

export async function getFuelCalibration(query: FuelCalibrationQuery) {
  await ensureDbSchema();

  const now = new Date();
  const from = new Date(now.getTime() - query.days * 24 * 60 * 60 * 1000);
  const fromBoundaryIso = from.toISOString();
  const toBoundaryIso = now.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? null;
  const policyVersion = query.policy_version ?? null;

  const rows = await executeRows<CalibrationDailyRow>(sql`
    select
      day,
      tenant_id,
      app_id,
      policy_version,
      event_count,
      k_latency_current,
      k_errors_current,
      k_energy_current,
      base_cost_points_total,
      fuel_points_total,
      usd_effective_total,
      penalty_latency_total,
      penalty_errors_total,
      penalty_energy_total,
      penalty_latency_p95,
      penalty_errors_p95,
      penalty_energy_p95,
      latency_excess_ratio_p95,
      error_signal_p95,
      energy_kwh_p95,
      delta_points_if_k_latency_plus_10pct,
      delta_points_if_k_errors_plus_10pct,
      delta_points_if_k_energy_plus_10pct,
      sensitivity_latency_share,
      sensitivity_errors_share,
      sensitivity_energy_share
    from fuel_policy_calibration_daily_v1
    where day >= cast(${fromBoundaryIso} as timestamptz)
      and day < cast(${toBoundaryIso} as timestamptz)
      and (${tenantId}::text is null or tenant_id = ${tenantId})
      and (${appId}::text is null or app_id = ${appId})
      and (${policyVersion}::text is null or policy_version = ${policyVersion})
    order by day desc, fuel_points_total desc
    limit ${query.limit}
  `);

  return {
    generated_at: now.toISOString(),
    window: {
      days: query.days,
      from: fromBoundaryIso,
      to: toBoundaryIso,
    },
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      policy_version: policyVersion,
    },
    items: rows.map((row) => ({
      day: toIso(row.day),
      tenant_id: row.tenant_id ?? null,
      app_id: row.app_id ?? null,
      policy_version: row.policy_version ?? 'unassigned',
      event_count: toNumber(row.event_count),
      k_current: {
        latency: toNumber(row.k_latency_current),
        errors: toNumber(row.k_errors_current),
        energy: toNumber(row.k_energy_current),
      },
      totals: {
        base_cost_points_total: toNumber(row.base_cost_points_total),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        penalty_latency_total: toNumber(row.penalty_latency_total),
        penalty_errors_total: toNumber(row.penalty_errors_total),
        penalty_energy_total: toNumber(row.penalty_energy_total),
      },
      p95: {
        penalty_latency: toNumber(row.penalty_latency_p95),
        penalty_errors: toNumber(row.penalty_errors_p95),
        penalty_energy: toNumber(row.penalty_energy_p95),
        latency_excess_ratio: toNumber(row.latency_excess_ratio_p95),
        error_signal: toNumber(row.error_signal_p95),
        energy_kwh: toNumber(row.energy_kwh_p95),
      },
      sensitivity: {
        delta_points_if_k_latency_plus_10pct: toNumber(row.delta_points_if_k_latency_plus_10pct),
        delta_points_if_k_errors_plus_10pct: toNumber(row.delta_points_if_k_errors_plus_10pct),
        delta_points_if_k_energy_plus_10pct: toNumber(row.delta_points_if_k_energy_plus_10pct),
        latency_share: toNumber(row.sensitivity_latency_share),
        errors_share: toNumber(row.sensitivity_errors_share),
        energy_share: toNumber(row.sensitivity_energy_share),
      },
    })),
  };
}

export async function getFuelOps(query: FuelOpsQuery) {
  await ensureDbSchema();

  const now = new Date();
  const from = new Date(now.getTime() - query.days * 24 * 60 * 60 * 1000);
  const fromBoundaryIso = from.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? null;
  const policyVersion = query.policy_version ?? null;

  const rows = await executeRows<FuelOpsRunRow>(sql`
    select
      id,
      job_name,
      evidence_date,
      tenant_id,
      app_id,
      policy_version,
      status,
      started_at,
      completed_at,
      metrics,
      evidence
    from fuel_ops_job_runs
    where created_at >= cast(${fromBoundaryIso} as timestamptz)
      and (${tenantId}::text is null or tenant_id = ${tenantId})
      and (${appId}::text is null or app_id = ${appId})
      and (${policyVersion}::text is null or policy_version = ${policyVersion})
    order by evidence_date desc, created_at desc
    limit ${query.limit}
  `);

  return {
    generated_at: now.toISOString(),
    window: {
      days: query.days,
      from: fromBoundaryIso,
      to: now.toISOString(),
    },
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      policy_version: policyVersion,
    },
    items: rows.map((row) => ({
      id: toNumber(row.id),
      job_name: row.job_name,
      evidence_date: toIso(row.evidence_date).slice(0, 10),
      tenant_id: row.tenant_id ?? null,
      app_id: row.app_id ?? null,
      policy_version: row.policy_version ?? null,
      status: row.status,
      started_at: toIso(row.started_at),
      completed_at: row.completed_at ? toIso(row.completed_at) : null,
      metrics: row.metrics ?? {},
      evidence: row.evidence ?? {},
    })),
  };
}

export async function materializeFuelOpsJobs(input?: {
  job_name?: 'baseline_snapshot' | 'alerts_snapshot' | 'baseline_and_alerts';
  reference_time?: string;
}) {
  await ensureDbSchema();

  const jobName = input?.job_name ?? 'baseline_and_alerts';
  const referenceTime = parseDateOrNull(input?.reference_time) ?? new Date();
  const rows = await executeRows<FuelOpsRunRow>(sql`
    select *
    from app.materialize_fuel_ops_jobs(
      ${jobName},
      cast(${referenceTime.toISOString()} as timestamptz)
    )
  `);

  return {
    materialized_at: new Date().toISOString(),
    job_name: jobName,
    reference_time: referenceTime.toISOString(),
    rows: rows.map((row) => ({
      id: toNumber(row.id),
      job_name: row.job_name,
      evidence_date: toIso(row.evidence_date).slice(0, 10),
      tenant_id: row.tenant_id ?? null,
      app_id: row.app_id ?? null,
      policy_version: row.policy_version ?? null,
      status: row.status,
      metrics: row.metrics ?? {},
      evidence: row.evidence ?? {},
    })),
  };
}
