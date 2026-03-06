import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getObsRunState, runIdSchema, runQuerySchema } from '@/lib/obs/events';

type Params = { params: Promise<{ runId: string }> };

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const { runId } = await params;
  const parsedRunId = runIdSchema.safeParse(runId);
  if (!parsedRunId.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid run_id');
  }

  const parsedQuery = runQuerySchema.safeParse({
    recent_limit: req.nextUrl.searchParams.get('recent_limit') ?? undefined,
  });
  if (!parsedQuery.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid query params', {
      issues: parsedQuery.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const runState = await getObsRunState(parsedRunId.data, parsedQuery.data);
  if (!runState) {
    return errorEnvelope(requestId, 404, 'NOT_FOUND', 'run not found');
  }
  return successEnvelope(requestId, runState);
}
