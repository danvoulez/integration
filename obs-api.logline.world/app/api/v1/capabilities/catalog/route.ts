import { NextRequest, NextResponse } from 'next/server';
import { requireObsScope } from '@/lib/auth/obs-scope';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { capabilitiesCatalogQuerySchema, getCapabilitiesCatalog } from '@/lib/obs/capabilities';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireObsScope(req, 'obs:read');
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const parsedQuery = capabilitiesCatalogQuerySchema.safeParse({
    service_id: req.nextUrl.searchParams.get('service_id') ?? undefined,
    surface: req.nextUrl.searchParams.get('surface') ?? undefined,
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
    const payload = await getCapabilitiesCatalog(parsedQuery.data);
    return successEnvelope(requestId, payload);
  } catch (error) {
    return errorEnvelope(requestId, 500, 'CATALOG_UNAVAILABLE', 'capability catalog is not available', {
      detail: error instanceof Error ? error.message : String(error),
    });
  }
}
