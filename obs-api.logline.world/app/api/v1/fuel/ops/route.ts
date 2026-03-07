import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { fuelOpsQuerySchema, getFuelOps, materializeFuelOpsJobs } from '@/lib/obs/fuel';
import { z } from 'zod';

const fuelOpsMaterializeSchema = z.object({
  job_name: z.enum(['baseline_snapshot', 'alerts_snapshot', 'baseline_and_alerts']).optional(),
  reference_time: z.string().trim().optional(),
});

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = fuelOpsQuerySchema.safeParse({
    days: req.nextUrl.searchParams.get('days') ?? undefined,
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
    tenant_id: req.nextUrl.searchParams.get('tenant_id') ?? undefined,
    app_id: req.nextUrl.searchParams.get('app_id') ?? undefined,
    policy_version: req.nextUrl.searchParams.get('policy_version') ?? undefined,
  });

  if (!parsedQuery.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid query params', {
      issues: parsedQuery.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  try {
    const payload = await getFuelOps(parsedQuery.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'FUEL_OPS_FAILED', 'failed to load fuel ops evidence', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:alerts:ack');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  let body: unknown;
  try {
    body = await req.json();
  } catch {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid JSON body');
  }

  const parsed = fuelOpsMaterializeSchema.safeParse(body ?? {});
  if (!parsed.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid fuel ops materialize payload', {
      issues: parsed.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  try {
    const payload = await materializeFuelOpsJobs(parsed.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'FUEL_OPS_MATERIALIZE_FAILED', 'failed to materialize fuel ops jobs', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
