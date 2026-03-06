import { createHash } from 'node:crypto';
import { asc, desc, eq, gte, inArray, or, sql } from 'drizzle-orm';
import { z } from 'zod';
import { db } from '@/db';
import { ensureDbSchema } from '@/db/bootstrap';
import { obsAlerts, obsEvents, obsRunState } from '@/db/schema';

const sourceEnum = z.enum([
  'code247',
  'linear',
  'github',
  'ci',
  'deploy',
  'llm-gateway',
  'logic',
  'obs-api',
  'edge-control',
]);

const isoTimestamp = z.string().refine((value) => {
  const date = new Date(value);
  return !Number.isNaN(date.getTime());
}, 'occurred_at must be a valid ISO timestamp');

const jsonPayloadObject = z.object({}).catchall(z.unknown());

export const ingestEventSchema = z.object({
  event_id: z.string().uuid(),
  event_type: z.string().min(1).max(128),
  occurred_at: isoTimestamp,
  source: sourceEnum,
  request_id: z.string().min(1).max(128),
  trace_id: z.string().min(1).max(128).nullable().optional(),
  parent_event_id: z.string().min(1).max(128).nullable().optional(),
  intention_id: z.string().min(1).max(128).nullable(),
  run_id: z.string().min(1).max(128).nullable(),
  issue_id: z.string().min(1).max(128).nullable(),
  pr_id: z.string().min(1).max(128).nullable(),
  deploy_id: z.string().min(1).max(128).nullable(),
  payload: jsonPayloadObject,
}).strict();

export const intentionIdSchema = z.string().trim().min(1).max(128);
export const runIdSchema = z.string().trim().min(1).max(128);
export const traceIdSchema = z.string().trim().min(1).max(128);

export const timelineQuerySchema = z.object({
  limit: z.coerce.number().int().min(1).max(500).default(100),
  order: z.enum(['asc', 'desc']).default('asc'),
  include_related: z.preprocess((value) => {
    if (value === undefined || value === null || value === '') return true;
    if (typeof value === 'string') {
      const normalized = value.trim().toLowerCase();
      if (normalized === 'true' || normalized === '1') return true;
      if (normalized === 'false' || normalized === '0') return false;
    }
    return value;
  }, z.boolean()).default(true),
});

export const runQuerySchema = z.object({
  recent_limit: z.coerce.number().int().min(1).max(200).default(20),
});

export const traceQuerySchema = z.object({
  limit: z.coerce.number().int().min(1).max(1000).default(300),
  order: z.enum(['asc', 'desc']).default('asc'),
});

export const summaryQuerySchema = z.object({
  window_minutes: z.coerce.number().int().min(5).max(1440).default(60),
  stale_run_minutes: z.coerce.number().int().min(5).max(10080).default(30),
  max_rows: z.coerce.number().int().min(100).max(10000).default(2000),
});

export const alertsOpenQuerySchema = z.object({
  window_minutes: z.coerce.number().int().min(5).max(10080).default(180),
  stale_run_minutes: z.coerce.number().int().min(5).max(10080).default(30),
  limit: z.coerce.number().int().min(1).max(500).default(100),
});

export const alertAckSchema = z.object({
  alert_id: z.string().trim().min(1).max(128),
  reason: z.string().trim().min(3).max(1000),
  actor: z.string().trim().min(1).max(128).optional(),
}).strict();

export type IngestEvent = z.infer<typeof ingestEventSchema>;
export type TimelineQuery = z.infer<typeof timelineQuerySchema>;
export type RunQuery = z.infer<typeof runQuerySchema>;
export type TraceQuery = z.infer<typeof traceQuerySchema>;
export type SummaryQuery = z.infer<typeof summaryQuerySchema>;
export type AlertsOpenQuery = z.infer<typeof alertsOpenQuerySchema>;
export type AlertAckInput = z.infer<typeof alertAckSchema>;

type ObsRow = typeof obsEvents.$inferSelect;
type ObsRunRow = typeof obsRunState.$inferSelect;
type ObsAlertRow = typeof obsAlerts.$inferSelect;

function payloadString(payload: Record<string, unknown>, key: string): string | null {
  const value = payload[key];
  if (typeof value === 'string' && value.trim().length > 0) return value;
  return null;
}

function payloadBoolean(payload: Record<string, unknown>, key: string): boolean | null {
  const value = payload[key];
  if (typeof value === 'boolean') return value;
  return null;
}

function payloadNumber(payload: Record<string, unknown>, key: string): number | null {
  const value = payload[key];
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function metadataString(payload: Record<string, unknown>, key: string): string | null {
  const metadata = payload.metadata;
  if (!metadata || typeof metadata !== 'object') return null;
  const value = (metadata as Record<string, unknown>)[key];
  if (typeof value === 'string' && value.trim().length > 0) return value;
  return null;
}

function resolveTraceId(row: Pick<ObsRow, 'trace_id' | 'payload'>): string | null {
  if (row.trace_id) return row.trace_id;
  return payloadString(row.payload as Record<string, unknown>, 'trace_id')
    ?? metadataString(row.payload as Record<string, unknown>, 'trace_id');
}

function resolveParentEventId(row: Pick<ObsRow, 'parent_event_id' | 'payload'>): string | null {
  if (row.parent_event_id) return row.parent_event_id;
  return payloadString(row.payload as Record<string, unknown>, 'parent_event_id')
    ?? metadataString(row.payload as Record<string, unknown>, 'parent_event_id');
}

function stageFromEvent(row: ObsRow): 'intake' | 'linear' | 'pr' | 'ci' | 'merge' | 'deploy' | 'other' {
  const type = row.event_type.toLowerCase();
  if (type.includes('intention') || type.includes('draft') || type.includes('claim')) return 'intake';
  if (row.source === 'linear' || type.includes('issue.')) return 'linear';
  if (type.includes('pr.') || type.includes('pull_request')) return 'pr';
  if (row.source === 'ci' || type.includes('ci.') || type.includes('check')) return 'ci';
  if (type.includes('merge.')) return 'merge';
  if (row.source === 'deploy' || type.includes('deploy.')) return 'deploy';
  return 'other';
}

function buildRoundTripStages(rows: ObsRow[]) {
  const order = ['intake', 'linear', 'pr', 'ci', 'merge', 'deploy'] as const;
  const stages: Record<string, string | null> = {
    intake: null,
    linear: null,
    pr: null,
    ci: null,
    merge: null,
    deploy: null,
  };

  for (const row of rows) {
    const stage = stageFromEvent(row);
    if (stage === 'other') continue;
    if (!stages[stage]) stages[stage] = row.occurred_at.toISOString();
  }

  const completed = order.every((k) => stages[k] !== null);
  return {
    stages,
    completed,
    last_completed_stage: [...order].reverse().find((k) => stages[k] !== null) ?? null,
  };
}

function mapEvent(row: ObsRow) {
  return {
    event_id: row.event_id,
    event_type: row.event_type,
    occurred_at: row.occurred_at,
    source: row.source,
    request_id: row.request_id,
    trace_id: resolveTraceId(row),
    parent_event_id: resolveParentEventId(row),
    intention_id: row.intention_id,
    run_id: row.run_id,
    issue_id: row.issue_id,
    pr_id: row.pr_id,
    deploy_id: row.deploy_id,
    payload: row.payload,
    ingested_at: row.ingested_at,
  };
}

function sortRows(rows: ObsRow[], order: 'asc' | 'desc'): ObsRow[] {
  const direction = order === 'asc' ? 1 : -1;
  return [...rows].sort((a, b) => {
    const ta = a.occurred_at.getTime();
    const tb = b.occurred_at.getTime();
    if (ta !== tb) return (ta - tb) * direction;
    return (a.ingested_at.getTime() - b.ingested_at.getTime()) * direction;
  });
}

function countBy(items: string[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const item of items) {
    out[item] = (out[item] ?? 0) + 1;
  }
  return out;
}

function toAlertId(code: string, scope: string): string {
  const digest = createHash('sha1').update(`${code}:${scope}`).digest('hex');
  return `alert_${digest.slice(0, 24)}`;
}

function minutesSince(start: Date, now: Date): number {
  return Math.max(0, Math.floor((now.getTime() - start.getTime()) / 60_000));
}

type AlertDraft = {
  alert_id: string;
  code: string;
  severity: 'warn' | 'critical';
  summary: string;
  details: Record<string, unknown>;
  source: string | null;
  intention_id: string | null;
  run_id: string | null;
  issue_id: string | null;
};

function deriveOperationalAlerts(
  recentRows: ObsRow[],
  runRows: ObsRunRow[],
  now: Date,
  staleCutoff: Date,
): AlertDraft[] {
  const out = new Map<string, AlertDraft>();

  for (const run of runRows) {
    if (run.last_ingested_at.getTime() >= staleCutoff.getTime()) continue;
    const staleMinutes = minutesSince(run.last_ingested_at, now);
    const severity: AlertDraft['severity'] = staleMinutes >= 4 * 60 ? 'critical' : 'warn';
    const alert: AlertDraft = {
      alert_id: toAlertId('run.stale', `run:${run.run_id}`),
      code: 'run.stale',
      severity,
      summary: `Run ${run.run_id} stale for ${staleMinutes}m`,
      details: {
        stale_minutes: staleMinutes,
        last_ingested_at: run.last_ingested_at.toISOString(),
        current_event_type: run.current_event_type,
        current_source: run.current_source,
      },
      source: run.current_source,
      intention_id: run.current_intention_id,
      run_id: run.run_id,
      issue_id: run.current_issue_id,
    };
    out.set(alert.alert_id, alert);
  }

  for (const row of recentRows) {
    const payload = row.payload as Record<string, unknown>;

    if (row.event_type === 'llm.request.completed' && payloadBoolean(payload, 'success') === false) {
      const provider = payloadString(payload, 'provider') ?? 'unknown';
      const model = payloadString(payload, 'model') ?? 'unknown';
      const alert: AlertDraft = {
        alert_id: toAlertId('llm.request.failed', `request:${row.request_id}`),
        code: 'llm.request.failed',
        severity: 'warn',
        summary: `LLM request failed (${provider}/${model})`,
        details: {
          request_id: row.request_id,
          occurred_at: row.occurred_at.toISOString(),
          provider,
          model,
          mode: payloadString(payload, 'mode'),
          error_message: payloadString(payload, 'error_message'),
          latency_ms: payloadNumber(payload, 'latency_ms'),
        },
        source: row.source,
        intention_id: row.intention_id,
        run_id: row.run_id,
        issue_id: row.issue_id,
      };
      out.set(alert.alert_id, alert);
      continue;
    }

    if (row.event_type === 'code247.intentions.synced') {
      const errorsCount = payloadNumber(payload, 'errors_count') ?? 0;
      if (errorsCount > 0) {
        const severity: AlertDraft['severity'] = errorsCount >= 3 ? 'critical' : 'warn';
        const alert: AlertDraft = {
          alert_id: toAlertId('code247.sync.errors', `request:${row.request_id}`),
          code: 'code247.sync.errors',
          severity,
          summary: `Code247 sync returned ${errorsCount} error(s)`,
          details: {
            request_id: row.request_id,
            occurred_at: row.occurred_at.toISOString(),
            errors_count: errorsCount,
            errors: Array.isArray(payload.errors) ? payload.errors : [],
            workspace: payloadString(payload, 'workspace'),
            project: payloadString(payload, 'project'),
          },
          source: row.source,
          intention_id: row.intention_id,
          run_id: row.run_id,
          issue_id: row.issue_id,
        };
        out.set(alert.alert_id, alert);
      }
    }
  }

  return [...out.values()];
}

function mapAlert(row: ObsAlertRow) {
  return {
    alert_id: row.alert_id,
    code: row.code,
    severity: row.severity,
    status: row.status,
    summary: row.summary,
    details: row.details,
    source: row.source,
    intention_id: row.intention_id,
    run_id: row.run_id,
    issue_id: row.issue_id,
    first_seen_at: row.first_seen_at,
    last_seen_at: row.last_seen_at,
    acked_at: row.acked_at,
    acked_by: row.acked_by,
    ack_reason: row.ack_reason,
    resolved_at: row.resolved_at,
    created_at: row.created_at,
    updated_at: row.updated_at,
  };
}

type AckObsAlertResult =
  | { state: 'not_found' }
  | { state: 'resolved'; alert: ReturnType<typeof mapAlert> }
  | { state: 'acked'; alert: ReturnType<typeof mapAlert> };

async function projectRunState(event: IngestEvent): Promise<void> {
  if (!event.run_id) return;

  const occurredAt = new Date(event.occurred_at);
  const existingRows = await db.select().from(obsRunState)
    .where(eq(obsRunState.run_id, event.run_id))
    .limit(1);

  const traceId = event.trace_id
    ?? payloadString(event.payload, 'trace_id')
    ?? metadataString(event.payload, 'trace_id');
  const parentEventId = event.parent_event_id
    ?? payloadString(event.payload, 'parent_event_id')
    ?? metadataString(event.payload, 'parent_event_id');

  const values = {
    run_id: event.run_id,
    current_event_id: event.event_id,
    current_event_type: event.event_type,
    current_occurred_at: occurredAt,
    current_source: event.source,
    current_request_id: event.request_id,
    current_trace_id: traceId,
    current_parent_event_id: parentEventId,
    current_intention_id: event.intention_id,
    current_issue_id: event.issue_id,
    current_pr_id: event.pr_id,
    current_deploy_id: event.deploy_id,
    current_payload: event.payload,
    last_ingested_at: new Date(),
    updated_at: new Date(),
  };

  const existing = existingRows[0];
  if (!existing) {
    await db.insert(obsRunState).values(values).onConflictDoNothing({
      target: obsRunState.run_id,
    });
    return;
  }

  // Keep projection monotonic by occurred_at (never regress to older state).
  if (existing.current_occurred_at.getTime() > occurredAt.getTime()) {
    return;
  }

  await db.update(obsRunState)
    .set(values)
    .where(eq(obsRunState.run_id, event.run_id));
}

export async function ingestObsEvent(event: IngestEvent): Promise<{ dedup: boolean; event_id: string }> {
  await ensureDbSchema();

  const traceId = event.trace_id
    ?? payloadString(event.payload, 'trace_id')
    ?? metadataString(event.payload, 'trace_id');
  const parentEventId = event.parent_event_id
    ?? payloadString(event.payload, 'parent_event_id')
    ?? metadataString(event.payload, 'parent_event_id');

  const inserted = await db.insert(obsEvents).values({
    event_id: event.event_id,
    event_type: event.event_type,
    occurred_at: new Date(event.occurred_at),
    source: event.source,
    request_id: event.request_id,
    trace_id: traceId,
    parent_event_id: parentEventId,
    intention_id: event.intention_id,
    run_id: event.run_id,
    issue_id: event.issue_id,
    pr_id: event.pr_id,
    deploy_id: event.deploy_id,
    payload: event.payload,
  }).onConflictDoNothing({
    target: obsEvents.event_id,
  }).returning({
    event_id: obsEvents.event_id,
  });

  // Maintain materialized run projection on every ingest (including dedup retries).
  await projectRunState(event);

  return {
    dedup: inserted.length === 0,
    event_id: event.event_id,
  };
}

export async function getObsTimeline(intention_id: string, query: TimelineQuery) {
  await ensureDbSchema();

  const primaryRows = await db.select().from(obsEvents)
    .where(eq(obsEvents.intention_id, intention_id))
    .orderBy(query.order === 'asc' ? asc(obsEvents.occurred_at) : desc(obsEvents.occurred_at))
    .limit(query.limit);

  let allRows = primaryRows;

  if (query.include_related && primaryRows.length > 0) {
    const requestIds = [...new Set(primaryRows.map((r) => r.request_id).filter((v): v is string => Boolean(v)))];
    const runIds = [...new Set(primaryRows.map((r) => r.run_id).filter((v): v is string => Boolean(v)))];

    if (requestIds.length > 0 || runIds.length > 0) {
      const where = [
        requestIds.length > 0 ? inArray(obsEvents.request_id, requestIds) : undefined,
        runIds.length > 0 ? inArray(obsEvents.run_id, runIds) : undefined,
      ].filter((clause): clause is NonNullable<typeof clause> => clause !== undefined);

      if (where.length > 0) {
        const relatedRows = await db.select().from(obsEvents)
          .where(or(...where))
          .orderBy(query.order === 'asc' ? asc(obsEvents.occurred_at) : desc(obsEvents.occurred_at))
          .limit(Math.min(query.limit * 2, 1000));

        const seen = new Set<string>();
        allRows = sortRows([...primaryRows, ...relatedRows], query.order).filter((row) => {
          if (seen.has(row.event_id)) return false;
          seen.add(row.event_id);
          return true;
        }).slice(0, query.limit);
      }
    }
  }

  const requestIds = [...new Set(allRows.map((row) => row.request_id))];
  const traceIds = [...new Set(allRows.map((row) => resolveTraceId(row)).filter((v): v is string => Boolean(v)))];

  return {
    intention_id,
    count: allRows.length,
    order: query.order,
    limit: query.limit,
    include_related: query.include_related,
    request_ids: requestIds,
    trace_ids: traceIds,
    round_trip: buildRoundTripStages(sortRows(allRows, 'asc')),
    items: allRows.map(mapEvent),
  };
}

export async function getObsRunState(run_id: string, query: RunQuery) {
  await ensureDbSchema();

  const projectionRows = await db.select().from(obsRunState)
    .where(eq(obsRunState.run_id, run_id))
    .limit(1);
  let projection = projectionRows[0];
  if (!projection) {
    const latestRows = await db.select().from(obsEvents)
      .where(eq(obsEvents.run_id, run_id))
      .orderBy(desc(obsEvents.occurred_at), desc(obsEvents.ingested_at))
      .limit(1);
    const latest = latestRows[0];
    if (!latest) return null;

    const fallbackValues = {
      run_id,
      current_event_id: latest.event_id,
      current_event_type: latest.event_type,
      current_occurred_at: latest.occurred_at,
      current_source: latest.source,
      current_request_id: latest.request_id,
      current_trace_id: resolveTraceId(latest),
      current_parent_event_id: resolveParentEventId(latest),
      current_intention_id: latest.intention_id,
      current_issue_id: latest.issue_id,
      current_pr_id: latest.pr_id,
      current_deploy_id: latest.deploy_id,
      current_payload: latest.payload,
      last_ingested_at: latest.ingested_at,
      updated_at: new Date(),
    };

    await db.insert(obsRunState).values(fallbackValues).onConflictDoNothing({
      target: obsRunState.run_id,
    });
    projection = fallbackValues;
  }

  const recentRows = await db.select().from(obsEvents)
    .where(eq(obsEvents.run_id, run_id))
    .orderBy(desc(obsEvents.occurred_at), desc(obsEvents.ingested_at))
    .limit(query.recent_limit);

  return {
    run_id,
    current_state: {
      event_id: projection.current_event_id,
      event_type: projection.current_event_type,
      occurred_at: projection.current_occurred_at,
      source: projection.current_source,
      request_id: projection.current_request_id,
      trace_id: projection.current_trace_id,
      parent_event_id: projection.current_parent_event_id,
      intention_id: projection.current_intention_id,
      issue_id: projection.current_issue_id,
      pr_id: projection.current_pr_id,
      deploy_id: projection.current_deploy_id,
      payload: projection.current_payload,
      ingested_at: projection.last_ingested_at,
    },
    recent_limit: query.recent_limit,
    recent_count: recentRows.length,
    recent_events: recentRows.map(mapEvent),
  };
}

export async function getObsTraceTree(trace_id: string, query: TraceQuery) {
  await ensureDbSchema();

  const rows = await db.select().from(obsEvents)
    .where(or(
      eq(obsEvents.trace_id, trace_id),
      sql`${obsEvents.payload} ->> 'trace_id' = ${trace_id}`,
      sql`${obsEvents.payload} -> 'metadata' ->> 'trace_id' = ${trace_id}`,
    ))
    .orderBy(query.order === 'asc' ? asc(obsEvents.occurred_at) : desc(obsEvents.occurred_at))
    .limit(query.limit);

  if (rows.length === 0) {
    return null;
  }

  const mapped = rows.map(mapEvent);
  const byId = new Map(mapped.map((node) => [node.event_id, node]));
  const childrenByParent = new Map<string, string[]>();
  const roots: string[] = [];

  for (const node of mapped) {
    if (node.parent_event_id && byId.has(node.parent_event_id)) {
      const children = childrenByParent.get(node.parent_event_id) ?? [];
      children.push(node.event_id);
      childrenByParent.set(node.parent_event_id, children);
    } else {
      roots.push(node.event_id);
    }
  }

  return {
    trace_id,
    count: mapped.length,
    order: query.order,
    limit: query.limit,
    roots: [...new Set(roots)],
    nodes: mapped.map((node) => ({
      ...node,
      children_event_ids: childrenByParent.get(node.event_id) ?? [],
    })),
  };
}

export async function getObsOpenAlerts(query: AlertsOpenQuery) {
  await ensureDbSchema();

  const now = new Date();
  const since = new Date(now.getTime() - (query.window_minutes * 60 * 1000));
  const staleCutoff = new Date(now.getTime() - (query.stale_run_minutes * 60 * 1000));

  const [recentRows, runRows] = await Promise.all([
    db.select().from(obsEvents)
      .where(gte(obsEvents.occurred_at, since))
      .orderBy(desc(obsEvents.occurred_at))
      .limit(5000),
    db.select().from(obsRunState)
      .orderBy(desc(obsRunState.last_ingested_at))
      .limit(5000),
  ]);

  const derivedAlerts = deriveOperationalAlerts(recentRows, runRows, now, staleCutoff);
  const derivedById = new Map(derivedAlerts.map((alert) => [alert.alert_id, alert]));
  const derivedIds = [...derivedById.keys()];

  if (derivedIds.length > 0) {
    const existingForDerived = await db.select().from(obsAlerts)
      .where(inArray(obsAlerts.alert_id, derivedIds));
    const existingById = new Map(existingForDerived.map((row) => [row.alert_id, row]));

    for (const alert of derivedAlerts) {
      const existing = existingById.get(alert.alert_id);
      if (existing) {
        await db.update(obsAlerts).set({
          code: alert.code,
          severity: alert.severity,
          status: existing.status === 'resolved' ? 'open' : existing.status,
          summary: alert.summary,
          details: alert.details,
          source: alert.source,
          intention_id: alert.intention_id,
          run_id: alert.run_id,
          issue_id: alert.issue_id,
          last_seen_at: now,
          resolved_at: null,
          updated_at: now,
        }).where(eq(obsAlerts.alert_id, alert.alert_id));
      } else {
        await db.insert(obsAlerts).values({
          alert_id: alert.alert_id,
          code: alert.code,
          severity: alert.severity,
          status: 'open',
          summary: alert.summary,
          details: alert.details,
          source: alert.source,
          intention_id: alert.intention_id,
          run_id: alert.run_id,
          issue_id: alert.issue_id,
          first_seen_at: now,
          last_seen_at: now,
          created_at: now,
          updated_at: now,
        }).onConflictDoNothing({
          target: obsAlerts.alert_id,
        });
      }
    }
  }

  const unresolved = await db.select().from(obsAlerts)
    .where(inArray(obsAlerts.status, ['open', 'acked']));
  for (const alert of unresolved) {
    if (derivedById.has(alert.alert_id)) continue;
    await db.update(obsAlerts).set({
      status: 'resolved',
      resolved_at: now,
      updated_at: now,
    }).where(eq(obsAlerts.alert_id, alert.alert_id));
  }

  const [openAlerts, countsRows] = await Promise.all([
    db.select().from(obsAlerts)
      .where(eq(obsAlerts.status, 'open'))
      .orderBy(desc(obsAlerts.last_seen_at))
      .limit(query.limit),
    db.select({
      open_count: sql<number>`count(*) filter (where ${obsAlerts.status} = 'open')::int`,
      acked_count: sql<number>`count(*) filter (where ${obsAlerts.status} = 'acked')::int`,
      resolved_count: sql<number>`count(*) filter (where ${obsAlerts.status} = 'resolved')::int`,
    }).from(obsAlerts),
  ]);

  const counts = countsRows[0] ?? { open_count: 0, acked_count: 0, resolved_count: 0 };
  return {
    generated_at: now.toISOString(),
    window: {
      from: since.toISOString(),
      to: now.toISOString(),
      minutes: query.window_minutes,
      stale_run_minutes: query.stale_run_minutes,
    },
    open_count: counts.open_count,
    acked_count: counts.acked_count,
    resolved_count: counts.resolved_count,
    limit: query.limit,
    items: openAlerts.map(mapAlert),
  };
}

export async function ackObsAlert(input: AlertAckInput): Promise<AckObsAlertResult> {
  await ensureDbSchema();

  const rows = await db.select().from(obsAlerts)
    .where(eq(obsAlerts.alert_id, input.alert_id))
    .limit(1);
  const current = rows[0];
  if (!current) return { state: 'not_found' };
  if (current.status === 'resolved') {
    return { state: 'resolved', alert: mapAlert(current) };
  }

  const now = new Date();
  const ackedBy = input.actor?.trim() || 'unknown';
  await db.update(obsAlerts).set({
    status: 'acked',
    acked_at: now,
    acked_by: ackedBy,
    ack_reason: input.reason,
    updated_at: now,
  }).where(eq(obsAlerts.alert_id, input.alert_id));

  const updatedRows = await db.select().from(obsAlerts)
    .where(eq(obsAlerts.alert_id, input.alert_id))
    .limit(1);
  const updated = updatedRows[0];
  if (!updated) return { state: 'not_found' };
  return { state: 'acked', alert: mapAlert(updated) };
}

export async function getObsDashboardSummary(query: SummaryQuery) {
  await ensureDbSchema();

  const now = new Date();
  const since = new Date(now.getTime() - (query.window_minutes * 60 * 1000));
  const staleCutoff = new Date(now.getTime() - (query.stale_run_minutes * 60 * 1000));

  const recentRows = await db.select().from(obsEvents)
    .where(gte(obsEvents.occurred_at, since))
    .orderBy(desc(obsEvents.occurred_at))
    .limit(query.max_rows);

  const totalRecentCountRows = await db.select({
    count: sql<number>`count(*)::int`,
  }).from(obsEvents).where(gte(obsEvents.occurred_at, since));
  const totalRecentCount = totalRecentCountRows[0]?.count ?? 0;

  const runs = await db.select().from(obsRunState)
    .orderBy(desc(obsRunState.last_ingested_at))
    .limit(query.max_rows);

  const staleRuns = runs.filter((run) => run.last_ingested_at.getTime() < staleCutoff.getTime());
  const sourceCounts = countBy(recentRows.map((row) => row.source));
  const stageCounts = countBy(recentRows.map((row) => stageFromEvent(row)));
  const typeCounts = countBy(recentRows.map((row) => row.event_type));
  const topEventTypes = Object.entries(typeCounts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 12)
    .map(([event_type, count]) => ({ event_type, count }));

  const healthReasons: string[] = [];
  if (totalRecentCount === 0) healthReasons.push('no_recent_events');
  if (staleRuns.length > 0) healthReasons.push('stale_runs_detected');

  return {
    generated_at: now.toISOString(),
    window: {
      from: since.toISOString(),
      to: now.toISOString(),
      minutes: query.window_minutes,
      stale_run_minutes: query.stale_run_minutes,
    },
    sampling: {
      max_rows: query.max_rows,
      sampled_events: recentRows.length,
      sampled_runs: runs.length,
      events_truncated: totalRecentCount > recentRows.length,
    },
    events: {
      recent_count: totalRecentCount,
      by_source: sourceCounts,
      by_stage: stageCounts,
      top_event_types: topEventTypes,
      unique_intentions: [...new Set(recentRows.map((row) => row.intention_id).filter((v): v is string => Boolean(v)))].length,
      unique_runs: [...new Set(recentRows.map((row) => row.run_id).filter((v): v is string => Boolean(v)))].length,
      unique_requests: [...new Set(recentRows.map((row) => row.request_id).filter((v): v is string => Boolean(v)))].length,
      unique_traces: [...new Set(recentRows.map((row) => resolveTraceId(row)).filter((v): v is string => Boolean(v)))].length,
    },
    runs: {
      projection_count: runs.length,
      stale_count: staleRuns.length,
      stale_run_ids: staleRuns.slice(0, 20).map((run) => run.run_id),
      by_source: countBy(runs.map((run) => run.current_source)),
    },
    health: {
      status: healthReasons.length > 0 ? 'degraded' : 'ok',
      reasons: healthReasons,
    },
  };
}
