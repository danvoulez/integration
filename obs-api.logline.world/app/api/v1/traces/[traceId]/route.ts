import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getObsTraceTree, traceIdSchema, traceQuerySchema } from '@/lib/obs/events';

type Params = { params: Promise<{ traceId: string }> };

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const { traceId } = await params;
  const parsedTraceId = traceIdSchema.safeParse(traceId);
  if (!parsedTraceId.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid trace_id');
  }

  const parsedQuery = traceQuerySchema.safeParse({
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
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

  const tree = await getObsTraceTree(parsedTraceId.data, parsedQuery.data);
  if (!tree) {
    return errorEnvelope(requestId, 404, 'NOT_FOUND', 'trace not found');
  }

  return successEnvelope(requestId, tree);
}
