import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { code247RunTimelineQuerySchema, getCode247RunTimeline } from '@/lib/obs/code247';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = code247RunTimelineQuerySchema.safeParse({
    days: req.nextUrl.searchParams.get('days') ?? undefined,
    jobs_limit: req.nextUrl.searchParams.get('jobs_limit') ?? undefined,
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
    tenant_id: req.nextUrl.searchParams.get('tenant_id') ?? undefined,
    app_id: req.nextUrl.searchParams.get('app_id') ?? undefined,
    job_id: req.nextUrl.searchParams.get('job_id') ?? undefined,
    issue_id: req.nextUrl.searchParams.get('issue_id') ?? undefined,
    order: req.nextUrl.searchParams.get('order') ?? undefined,
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
    const payload = await getCode247RunTimeline(parsedQuery.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'CODE247_RUN_TIMELINE_FAILED', 'failed to compute code247 run timeline', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
