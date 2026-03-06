import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { fuelDashboardQuerySchema, getFuelDashboard } from '@/lib/obs/fuel';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = fuelDashboardQuerySchema.safeParse({
    realtime_window_minutes: req.nextUrl.searchParams.get('realtime_window_minutes') ?? undefined,
    max_rows: req.nextUrl.searchParams.get('max_rows') ?? undefined,
    preset: req.nextUrl.searchParams.get('preset') ?? undefined,
    from: req.nextUrl.searchParams.get('from') ?? undefined,
    to: req.nextUrl.searchParams.get('to') ?? undefined,
    tenant_id: req.nextUrl.searchParams.get('tenant_id') ?? undefined,
    app_id: req.nextUrl.searchParams.get('app_id') ?? undefined,
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
    const dashboard = await getFuelDashboard(parsedQuery.data);
    return successEnvelope(requestId, dashboard);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'FUEL_DASHBOARD_FAILED', 'failed to compute fuel dashboard', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
