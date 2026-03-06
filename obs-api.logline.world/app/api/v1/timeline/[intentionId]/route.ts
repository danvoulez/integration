import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getObsTimeline, intentionIdSchema, timelineQuerySchema } from '@/lib/obs/events';

type Params = { params: Promise<{ intentionId: string }> };

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const { intentionId } = await params;
  const parsedIntentionId = intentionIdSchema.safeParse(intentionId);
  if (!parsedIntentionId.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid intention_id');
  }

  const parsedQuery = timelineQuerySchema.safeParse({
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
    order: req.nextUrl.searchParams.get('order') ?? undefined,
    include_related: req.nextUrl.searchParams.get('include_related') ?? undefined,
  });
  if (!parsedQuery.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid query params', {
      issues: parsedQuery.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const timeline = await getObsTimeline(parsedIntentionId.data, parsedQuery.data);
  return successEnvelope(requestId, timeline);
}
