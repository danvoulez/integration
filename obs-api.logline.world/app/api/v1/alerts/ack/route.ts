import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { ackObsAlert, alertAckSchema } from '@/lib/obs/events';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:alerts:ack');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const body = await req.json().catch(() => null) as unknown;
  if (body === null) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid JSON body');
  }

  const parsed = alertAckSchema.safeParse(body);
  if (!parsed.success) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'invalid alert ack payload', {
      issues: parsed.error.issues.map((issue) => ({
        path: issue.path.join('.'),
        message: issue.message,
      })),
    });
  }

  const actor = parsed.data.actor ?? auth.claims.sub;
  const result = await ackObsAlert({
    alert_id: parsed.data.alert_id,
    reason: parsed.data.reason,
    actor,
  });

  if (result.state === 'not_found') {
    return errorEnvelope(requestId, 404, 'NOT_FOUND', 'alert not found');
  }
  if (result.state === 'resolved') {
    return errorEnvelope(requestId, 409, 'CONFLICT', 'alert already resolved', {
      alert: result.alert,
    });
  }

  return successEnvelope(requestId, {
    ok: true,
    state: result.state,
    alert: result.alert,
  });
}
