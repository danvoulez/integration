import { db } from '@/db';
import { ensureDbSchema } from '@/db/bootstrap';
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
  source: z.string().trim().min(1).max(128).optional(),
  provider: z.string().trim().min(1).max(128).optional(),
  model: z.string().trim().min(1).max(128).optional(),
});

export type FuelReconciliationQuery = z.infer<typeof fuelReconciliationQuerySchema>;

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

async function queryRealtimeWithBaseline(
  tenantId: string | null,
  appId: string | null,
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
    ), latest as (
      select max(window_start) as window_start from filtered
    )
    select
      f.window_start,
      f.tenant_id,
      f.app_id,
      f.source,
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
      group by 1, 2, 3, 4
    ), latest as (
      select max(window_start) as window_start from binned
    )
    select
      b.window_start,
      b.tenant_id,
      b.app_id,
      b.source,
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

  let realtimePayload: { series: RealtimeSeriesRow[]; movers: RealtimeTopMoverRow[]; baselineReady: boolean };
  try {
    realtimePayload = await queryRealtimeWithBaseline(tenantId, appId, query.realtime_window_minutes, query.max_rows);
  } catch (error) {
    if (!isUndefinedRelationError(error)) throw error;
    realtimePayload = await queryRealtimeWithoutBaseline(tenantId, appId, query.realtime_window_minutes, query.max_rows);
  }

  const [totalsRows, byAppRows, bySourceRows, providerModelRows, dailyRows, driftRows] = await Promise.all([
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
      group by date_trunc('day', day)
      order by day asc
    `),
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
  const source = query.source ?? null;
  const provider = query.provider ?? null;
  const model = query.model ?? null;

  const [coverageRows, driftRows, byAppRows, bySourceRows, byProviderModelRows] = await Promise.all([
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
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
    `),
    executeRows<ReconciliationDriftRow>(sql`
      select
        date_trunc('day', fe.occurred_at) as day,
        count(*)::bigint as event_count,
        sum(coalesce(fv.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fv.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fv.usd_settled, fv.usd_estimated, 0))::numeric as usd_effective_total,
        sum(coalesce(fv.usd_settled, 0) - coalesce(fv.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_events fe
      left join fuel_valuations fv on fv.event_id = fe.event_id
      where fe.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fe.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
      group by date_trunc('day', fe.occurred_at)
      order by day desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByGroupRow>(sql`
      select
        fe.app_id as key,
        count(*)::bigint as event_count,
        sum(case when fv.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fv.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fv.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fv.usd_settled, fv.usd_estimated, 0))::numeric as usd_effective_total,
        sum(coalesce(fv.usd_settled, 0) - coalesce(fv.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_events fe
      left join fuel_valuations fv on fv.event_id = fe.event_id
      where fe.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fe.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
      group by fe.app_id
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByGroupRow>(sql`
      select
        fe.source as key,
        count(*)::bigint as event_count,
        sum(case when fv.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fv.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fv.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fv.usd_settled, fv.usd_estimated, 0))::numeric as usd_effective_total,
        sum(coalesce(fv.usd_settled, 0) - coalesce(fv.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_events fe
      left join fuel_valuations fv on fv.event_id = fe.event_id
      where fe.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fe.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
      group by fe.source
      order by usd_effective_total desc
      limit ${query.limit}
    `),
    executeRows<ReconciliationByProviderModelRow>(sql`
      select
        coalesce(fe.metadata->>'provider', 'unknown') as provider,
        coalesce(fe.metadata->>'model', 'unknown') as model,
        count(*)::bigint as event_count,
        sum(case when fv.usd_settled is not null then 1 else 0 end)::bigint as settled_count,
        sum(coalesce(fv.usd_estimated, 0))::numeric as usd_estimated_total,
        sum(coalesce(fv.usd_settled, 0))::numeric as usd_settled_total,
        sum(coalesce(fv.usd_settled, fv.usd_estimated, 0))::numeric as usd_effective_total,
        sum(coalesce(fv.usd_settled, 0) - coalesce(fv.usd_estimated, 0))::numeric as usd_drift_total
      from fuel_events fe
      left join fuel_valuations fv on fv.event_id = fe.event_id
      where fe.occurred_at >= cast(${fromBoundaryIso} as timestamptz)
        and fe.occurred_at < cast(${toBoundaryIso} as timestamptz)
        and (${tenantId}::text is null or fe.tenant_id = ${tenantId})
        and (${appId}::text is null or fe.app_id = ${appId})
        and (${source}::text is null or fe.source = ${source})
        and (${provider}::text is null or fe.metadata->>'provider' = ${provider})
        and (${model}::text is null or fe.metadata->>'model' = ${model})
      group by coalesce(fe.metadata->>'provider', 'unknown'), coalesce(fe.metadata->>'model', 'unknown')
      order by usd_effective_total desc
      limit ${query.limit}
    `),
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
  };
}
