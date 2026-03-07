import { db } from '@/db';
import { ensureDbSchema } from '@/db/bootstrap';
import { SQL, sql } from 'drizzle-orm';
import { z } from 'zod';

const stageFamilySchema = z.enum(['plan', 'code', 'review', 'ci', 'merge', 'deploy']);

export const code247StageTelemetryQuerySchema = z.object({
  days: z.coerce.number().int().min(1).max(30).default(14),
  limit: z.coerce.number().int().min(1).max(100).default(20),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).default('code247'),
  issue_id: z.string().trim().min(1).max(128).optional(),
  stage: stageFamilySchema.optional(),
});

export type Code247StageTelemetryQuery = z.infer<typeof code247StageTelemetryQuerySchema>;

export const code247RunTimelineQuerySchema = z.object({
  days: z.coerce.number().int().min(1).max(30).default(14),
  jobs_limit: z.coerce.number().int().min(1).max(100).default(20),
  limit: z.coerce.number().int().min(1).max(1000).default(500),
  tenant_id: z.string().trim().min(1).max(128).optional(),
  app_id: z.string().trim().min(1).max(128).default('code247'),
  job_id: z.string().trim().min(1).max(128).optional(),
  issue_id: z.string().trim().min(1).max(128).optional(),
  order: z.enum(['asc', 'desc']).default('desc'),
});

export type Code247RunTimelineQuery = z.infer<typeof code247RunTimelineQuerySchema>;

type StageAggregateRow = {
  stage_family: string | null;
  raw_stage_count: string | number | null;
  event_count: string | number | null;
  run_count: string | number | null;
  avg_duration_ms: string | number | null;
  p95_duration_ms: string | number | null;
  max_duration_ms: string | number | null;
  failure_count: string | number | null;
  fuel_points_total: string | number | null;
  fuel_points_avg: string | number | null;
  usd_effective_total: string | number | null;
  usd_effective_avg: string | number | null;
  last_event_at: string | Date | null;
};

type StageDailyRow = {
  day: string | Date;
  stage_family: string | null;
  event_count: string | number | null;
  avg_duration_ms: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
};

type SlowestRow = {
  event_id: string;
  job_id: string;
  issue_id: string | null;
  stage_family: string | null;
  raw_stage: string | null;
  model_used: string | null;
  duration_ms: string | number | null;
  fuel_points_total: string | number | null;
  usd_effective_total: string | number | null;
  outcome: string | null;
  occurred_at: string | Date;
};

type TimelineJobRow = {
  job_id: string;
  issue_id: string;
  status: string;
  retries: string | number | null;
  last_error: string | null;
  created_at: string | Date;
  updated_at: string | Date;
};

type TimelineEntryRow = {
  job_id: string;
  issue_id: string;
  record_type: string;
  stage: string | null;
  status: string | null;
  event_type: string | null;
  detail: string | null;
  duration_ms: string | number | null;
  model_used: string | null;
  outcome: string | null;
  occurred_at: string | Date;
};

const STAGE_ORDER = ['plan', 'code', 'review', 'ci', 'merge', 'deploy'] as const;

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

export async function getCode247StageTelemetry(query: Code247StageTelemetryQuery) {
  await ensureDbSchema();

  const now = new Date();
  const from = new Date(now.getTime() - query.days * 24 * 60 * 60 * 1000);
  const fromIso = from.toISOString();
  const toIsoBoundary = now.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? 'code247';
  const issueId = query.issue_id ?? null;
  const stage = query.stage ?? null;

  const stageFamilyExpr = sql<string>`
    case
      when lower(coalesce(ce.stage, '')) in ('planning', 'plan') then 'plan'
      when lower(coalesce(ce.stage, '')) in ('coding', 'code') then 'code'
      when lower(coalesce(ce.stage, '')) in ('reviewing', 'review') then 'review'
      when lower(coalesce(ce.stage, '')) in ('validating', 'validate', 'ci') then 'ci'
      when lower(coalesce(ce.stage, '')) in ('committing', 'commit', 'merge') then 'merge'
      when lower(coalesce(ce.stage, '')) in ('deploy', 'deploying') then 'deploy'
      else lower(coalesce(ce.stage, 'unknown'))
    end
  `;

  const baseFiltered = sql`
    with stage_events as (
      select
        ce.event_id,
        ce.job_id,
        ce.stage as raw_stage,
        ${stageFamilyExpr} as stage_family,
        coalesce(ce.metadata->>'issue_id', '') as issue_id,
        ce.model_used,
        ce.duration_ms,
        coalesce(ce.metadata->>'outcome', 'ok') as outcome,
        ce.occurred_at,
        fp.fuel_points_total,
        fp.usd_effective
      from code247_events ce
      left join fuel_points_v1 fp on fp.event_id = concat('fuel:', ce.event_id)
      where ce.occurred_at >= cast(${fromIso} as timestamptz)
        and ce.occurred_at < cast(${toIsoBoundary} as timestamptz)
        and (${tenantId}::text is null or ce.tenant_id = ${tenantId})
        and (${appId}::text is null or ce.app_id = ${appId})
        and (${issueId}::text is null or ce.metadata->>'issue_id' = ${issueId})
        and (${stage}::text is null or ${stageFamilyExpr} = ${stage})
    )
  `;

  const emptyPayload = {
    generated_at: now.toISOString(),
    window: {
      days: query.days,
      from: fromIso,
      to: toIsoBoundary,
    },
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      issue_id: issueId,
      stage,
    },
    totals: {
      event_count: 0,
      run_count: 0,
      failure_count: 0,
      duration_ms_avg: 0,
      duration_ms_p95: 0,
      fuel_points_total: 0,
      usd_effective_total: 0,
    },
    by_stage: STAGE_ORDER.map((name) => ({
      stage: name,
      raw_stage_count: 0,
      event_count: 0,
      run_count: 0,
      failure_count: 0,
      failure_rate: 0,
      latency: {
        avg_ms: 0,
        p95_ms: 0,
        max_ms: 0,
      },
      cost: {
        fuel_points_total: 0,
        fuel_points_avg: 0,
        usd_effective_total: 0,
        usd_effective_avg: 0,
      },
      last_event_at: null as string | null,
    })),
    daily: [] as Array<{
      day: string;
      stage: string;
      event_count: number;
      latency: { avg_ms: number };
      cost: { fuel_points_total: number; usd_effective_total: number };
    }>,
    slowest: [] as Array<{
      event_id: string;
      job_id: string;
      issue_id: string | null;
      stage: string;
      raw_stage: string | null;
      model_used: string | null;
      duration_ms: number;
      fuel_points_total: number;
      usd_effective_total: number;
      outcome: string | null;
      occurred_at: string;
    }>,
  };

  try {
    const [aggregateRows, dailyRows, slowestRows, totalsRows] = await Promise.all([
      executeRows<StageAggregateRow>(sql`
        ${baseFiltered}
        select
          stage_family,
          count(distinct raw_stage)::bigint as raw_stage_count,
          count(*)::bigint as event_count,
          count(distinct job_id)::bigint as run_count,
          avg(duration_ms)::numeric as avg_duration_ms,
          percentile_cont(0.95) within group (order by duration_ms)
            filter (where duration_ms is not null)::numeric as p95_duration_ms,
          max(duration_ms)::integer as max_duration_ms,
          sum(case when outcome <> 'ok' then 1 else 0 end)::bigint as failure_count,
          sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
          avg(coalesce(fuel_points_total, 0))::numeric as fuel_points_avg,
          sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
          avg(coalesce(usd_effective, 0))::numeric as usd_effective_avg,
          max(occurred_at) as last_event_at
        from stage_events
        group by stage_family
        order by array_position(array['plan', 'code', 'review', 'ci', 'merge', 'deploy'], stage_family), stage_family
      `),
      executeRows<StageDailyRow>(sql`
        ${baseFiltered}
        select
          date_trunc('day', occurred_at) as day,
          stage_family,
          count(*)::bigint as event_count,
          avg(duration_ms)::numeric as avg_duration_ms,
          sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
          sum(coalesce(usd_effective, 0))::numeric as usd_effective_total
        from stage_events
        group by date_trunc('day', occurred_at), stage_family
        order by day desc, array_position(array['plan', 'code', 'review', 'ci', 'merge', 'deploy'], stage_family), stage_family
      `),
      executeRows<SlowestRow>(sql`
        ${baseFiltered}
        select
          event_id,
          job_id,
          nullif(issue_id, '') as issue_id,
          stage_family,
          raw_stage,
          model_used,
          duration_ms,
          coalesce(fuel_points_total, 0)::numeric as fuel_points_total,
          coalesce(usd_effective, 0)::numeric as usd_effective_total,
          outcome,
          occurred_at
        from stage_events
        order by duration_ms desc nulls last, occurred_at desc
        limit ${query.limit}
      `),
      executeRows<{
        event_count: string | number | null;
        run_count: string | number | null;
        failure_count: string | number | null;
        duration_ms_avg: string | number | null;
        duration_ms_p95: string | number | null;
        fuel_points_total: string | number | null;
        usd_effective_total: string | number | null;
      }>(sql`
        ${baseFiltered}
        select
          count(*)::bigint as event_count,
          count(distinct job_id)::bigint as run_count,
          sum(case when outcome <> 'ok' then 1 else 0 end)::bigint as failure_count,
          avg(duration_ms)::numeric as duration_ms_avg,
          percentile_cont(0.95) within group (order by duration_ms)
            filter (where duration_ms is not null)::numeric as duration_ms_p95,
          sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
          sum(coalesce(usd_effective, 0))::numeric as usd_effective_total
        from stage_events
      `),
    ]);

    const aggregateByStage = new Map(aggregateRows.map((row) => [row.stage_family ?? 'unknown', row]));
    const totals = totalsRows[0] ?? {
      event_count: 0,
      run_count: 0,
      failure_count: 0,
      duration_ms_avg: 0,
      duration_ms_p95: 0,
      fuel_points_total: 0,
      usd_effective_total: 0,
    };

    return {
      generated_at: now.toISOString(),
      window: {
        days: query.days,
        from: fromIso,
        to: toIsoBoundary,
      },
      filters: {
        tenant_id: tenantId,
        app_id: appId,
        issue_id: issueId,
        stage,
      },
      totals: {
        event_count: toNumber(totals.event_count),
        run_count: toNumber(totals.run_count),
        failure_count: toNumber(totals.failure_count),
        duration_ms_avg: toNumber(totals.duration_ms_avg),
        duration_ms_p95: toNumber(totals.duration_ms_p95),
        fuel_points_total: toNumber(totals.fuel_points_total),
        usd_effective_total: toNumber(totals.usd_effective_total),
      },
      by_stage: STAGE_ORDER.map((stageName) => {
        const row = aggregateByStage.get(stageName);
        const eventCount = toNumber(row?.event_count);
        const failureCount = toNumber(row?.failure_count);
        return {
          stage: stageName,
          raw_stage_count: toNumber(row?.raw_stage_count),
          event_count: eventCount,
          run_count: toNumber(row?.run_count),
          failure_count: failureCount,
          failure_rate: eventCount > 0 ? failureCount / eventCount : 0,
          latency: {
            avg_ms: toNumber(row?.avg_duration_ms),
            p95_ms: toNumber(row?.p95_duration_ms),
            max_ms: toNumber(row?.max_duration_ms),
          },
          cost: {
            fuel_points_total: toNumber(row?.fuel_points_total),
            fuel_points_avg: toNumber(row?.fuel_points_avg),
            usd_effective_total: toNumber(row?.usd_effective_total),
            usd_effective_avg: toNumber(row?.usd_effective_avg),
          },
          last_event_at: row?.last_event_at ? toIso(row.last_event_at) : null,
        };
      }),
      daily: dailyRows.map((row) => ({
        day: toIso(row.day),
        stage: row.stage_family ?? 'unknown',
        event_count: toNumber(row.event_count),
        latency: {
          avg_ms: toNumber(row.avg_duration_ms),
        },
        cost: {
          fuel_points_total: toNumber(row.fuel_points_total),
          usd_effective_total: toNumber(row.usd_effective_total),
        },
      })),
      slowest: slowestRows.map((row) => ({
        event_id: row.event_id,
        job_id: row.job_id,
        issue_id: row.issue_id,
        stage: row.stage_family ?? 'unknown',
        raw_stage: row.raw_stage,
        model_used: row.model_used,
        duration_ms: toNumber(row.duration_ms),
        fuel_points_total: toNumber(row.fuel_points_total),
        usd_effective_total: toNumber(row.usd_effective_total),
        outcome: row.outcome,
        occurred_at: toIso(row.occurred_at),
      })),
    };
  } catch (error) {
    if (isUndefinedRelationError(error)) {
      return emptyPayload;
    }
    throw error;
  }
}

export async function getCode247RunTimeline(query: Code247RunTimelineQuery) {
  await ensureDbSchema();

  const now = new Date();
  const from = new Date(now.getTime() - query.days * 24 * 60 * 60 * 1000);
  const fromIso = from.toISOString();
  const tenantId = query.tenant_id ?? null;
  const appId = query.app_id ?? 'code247';
  const jobId = query.job_id ?? null;
  const issueId = query.issue_id ?? null;
  const orderSql = query.order === 'asc' ? sql`asc` : sql`desc`;
  const scopedJobsLimit = jobId || issueId ? Math.max(query.jobs_limit, 100) : query.jobs_limit;

  const filteredJobs = sql`
    with filtered_jobs as (
      select
        j.id,
        j.issue_id,
        j.status,
        j.payload,
        j.retries,
        j.last_error,
        j.created_at,
        j.updated_at
      from code247_jobs j
      where j.updated_at >= cast(${fromIso} as timestamptz)
        and (${tenantId}::text is null or j.tenant_id = ${tenantId})
        and (${appId}::text is null or j.app_id = ${appId})
        and (${jobId}::text is null or j.id = ${jobId})
        and (${issueId}::text is null or j.issue_id = ${issueId})
      order by j.updated_at desc
      limit ${scopedJobsLimit}
    )
  `;

  const emptyPayload = {
    generated_at: now.toISOString(),
    window: {
      days: query.days,
      from: fromIso,
      to: now.toISOString(),
    },
    filters: {
      tenant_id: tenantId,
      app_id: appId,
      job_id: jobId,
      issue_id: issueId,
      order: query.order,
    },
    jobs_limit: query.jobs_limit,
    limit: query.limit,
    job_count: 0,
    jobs: [] as Array<{
      job_id: string;
      issue_id: string;
      status: string;
      retries: number;
      last_error: string | null;
      created_at: string;
      updated_at: string;
    }>,
    items: [] as Array<{
      job_id: string;
      issue_id: string;
      record_type: string;
      stage: string | null;
      status: string | null;
      event_type: string | null;
      detail: string | null;
      duration_ms: number;
      model_used: string | null;
      outcome: string | null;
      occurred_at: string;
    }>,
  };

  try {
    const [jobs, items] = await Promise.all([
      executeRows<TimelineJobRow>(sql`
        ${filteredJobs}
        select
          id as job_id,
          issue_id,
          status,
          retries,
          last_error,
          created_at,
          updated_at
        from filtered_jobs
        order by updated_at desc
      `),
      executeRows<TimelineEntryRow>(sql`
        ${filteredJobs}
        select *
        from (
          select
            j.id as job_id,
            j.issue_id,
            'job_created'::text as record_type,
            j.status as stage,
            j.status as status,
            'code247.job.created'::text as event_type,
            j.payload::text as detail,
            null::integer as duration_ms,
            null::text as model_used,
            null::text as outcome,
            j.created_at as occurred_at
          from filtered_jobs j

          union all

          select
            j.id as job_id,
            j.issue_id,
            'job_status'::text as record_type,
            j.status as stage,
            j.status as status,
            'code247.job.status'::text as event_type,
            j.last_error as detail,
            null::integer as duration_ms,
            null::text as model_used,
            null::text as outcome,
            j.updated_at as occurred_at
          from filtered_jobs j

          union all

          select
            c.job_id,
            j.issue_id,
            'checkpoint'::text as record_type,
            c.stage,
            j.status,
            null::text as event_type,
            c.data as detail,
            null::integer as duration_ms,
            null::text as model_used,
            null::text as outcome,
            c.created_at as occurred_at
          from code247_checkpoints c
          join filtered_jobs j on j.id = c.job_id

          union all

          select
            e.job_id,
            j.issue_id,
            'execution_event'::text as record_type,
            e.stage,
            j.status,
            e.event_type,
            coalesce(e.output, e.input) as detail,
            e.duration_ms,
            e.model_used,
            e.metadata->>'outcome' as outcome,
            e.occurred_at as occurred_at
          from code247_events e
          join filtered_jobs j on j.id = e.job_id
        ) timeline
        order by occurred_at ${orderSql}, record_type asc
        limit ${query.limit}
      `),
    ]);

    return {
      generated_at: now.toISOString(),
      window: {
        days: query.days,
        from: fromIso,
        to: now.toISOString(),
      },
      filters: {
        tenant_id: tenantId,
        app_id: appId,
        job_id: jobId,
        issue_id: issueId,
        order: query.order,
      },
      jobs_limit: query.jobs_limit,
      limit: query.limit,
      job_count: jobs.length,
      jobs: jobs.map((row) => ({
        job_id: row.job_id,
        issue_id: row.issue_id,
        status: row.status,
        retries: toNumber(row.retries),
        last_error: row.last_error,
        created_at: toIso(row.created_at),
        updated_at: toIso(row.updated_at),
      })),
      items: items.map((row) => ({
        job_id: row.job_id,
        issue_id: row.issue_id,
        record_type: row.record_type,
        stage: row.stage,
        status: row.status,
        event_type: row.event_type,
        detail: row.detail,
        duration_ms: toNumber(row.duration_ms),
        model_used: row.model_used,
        outcome: row.outcome,
        occurred_at: toIso(row.occurred_at),
      })),
    };
  } catch (error) {
    if (isUndefinedRelationError(error)) {
      return emptyPayload;
    }
    throw error;
  }
}
