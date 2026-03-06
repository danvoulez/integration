import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getObsDashboardSummary, summaryQuerySchema } from '@/lib/obs/events';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = summaryQuerySchema.safeParse({
    window_minutes: req.nextUrl.searchParams.get('window_minutes') ?? undefined,
    stale_run_minutes: req.nextUrl.searchParams.get('stale_run_minutes') ?? undefined,
    max_rows: req.nextUrl.searchParams.get('max_rows') ?? undefined,
  });
  if (!parsedQuery.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid query params', {
      issues: parsedQuery.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const summary = await getObsDashboardSummary(parsedQuery.data);
  return successEnvelope(requestId, summary);
}
