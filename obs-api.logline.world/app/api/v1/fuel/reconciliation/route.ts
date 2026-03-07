import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { fuelReconciliationQuerySchema, getFuelReconciliation } from '@/lib/obs/fuel';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = fuelReconciliationQuerySchema.safeParse({
    days: req.nextUrl.searchParams.get('days') ?? undefined,
    limit: req.nextUrl.searchParams.get('limit') ?? undefined,
    tenant_id: req.nextUrl.searchParams.get('tenant_id') ?? undefined,
    app_id: req.nextUrl.searchParams.get('app_id') ?? undefined,
    policy_version: req.nextUrl.searchParams.get('policy_version') ?? undefined,
    precision_level: req.nextUrl.searchParams.get('precision_level') ?? undefined,
    source: req.nextUrl.searchParams.get('source') ?? undefined,
    provider: req.nextUrl.searchParams.get('provider') ?? undefined,
    model: req.nextUrl.searchParams.get('model') ?? undefined,
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
    const payload = await getFuelReconciliation(parsedQuery.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'FUEL_RECONCILIATION_FAILED', 'failed to compute fuel reconciliation', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
