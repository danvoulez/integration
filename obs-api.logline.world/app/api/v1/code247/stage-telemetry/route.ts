import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { code247StageTelemetryQuerySchema, getCode247StageTelemetry } from '@/lib/obs/code247';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = code247StageTelemetryQuerySchema.safeParse({
    days: req.nextUrl.searchParams.get('days') ?? undefined,
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
    tenant_id: req.nextUrl.searchParams.get('tenant_id') ?? undefined,
    app_id: req.nextUrl.searchParams.get('app_id') ?? undefined,
    issue_id: req.nextUrl.searchParams.get('issue_id') ?? undefined,
    stage: req.nextUrl.searchParams.get('stage') ?? undefined,
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
    const payload = await getCode247StageTelemetry(parsedQuery.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'CODE247_STAGE_TELEMETRY_FAILED', 'failed to compute code247 stage telemetry', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
