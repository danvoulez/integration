import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { alertsOpenQuerySchema, getObsOpenAlerts } from '@/lib/obs/events';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = alertsOpenQuerySchema.safeParse({
    window_minutes: req.nextUrl.searchParams.get('window_minutes') ?? undefined,
    stale_run_minutes: req.nextUrl.searchParams.get('stale_run_minutes') ?? undefined,
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
  });
  if (!parsedQuery.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid query params', {
      issues: parsedQuery.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const alerts = await getObsOpenAlerts(parsedQuery.data);
  return successEnvelope(requestId, alerts);
}
