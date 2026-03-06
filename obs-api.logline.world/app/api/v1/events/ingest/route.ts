import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { ingestEventSchema, ingestObsEvent } from '@/lib/obs/events';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:ingest');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const body = await req.json().catch(() => null) as unknown;
  if (body === null) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid JSON body');
  }

  const parsed = ingestEventSchema.safeParse(body);
  if (!parsed.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid event envelope', {
      issues: parsed.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const event = parsed.data;
  const result = await ingestObsEvent(event);
  return successEnvelope(requestId, {
    ok: true,
    accepted: true,
    dedup: result.dedup,
    event_id: result.event_id,
  }, result.dedup ? 200 : 201);
}
