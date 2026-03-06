import { NextRequest, NextResponse } from 'next/server';
import { and, eq } from 'drizzle-orm';
import { db } from '@/db';
import { userProviderKeys } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getUserFromAuthHeader } from '@/lib/auth/supabase-server';

type Params = { params: Promise<{ appId: string }> };

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const authUser = await getUserFromAuthHeader(req.headers.get('authorization'));
  if (!authUser) return errorEnvelope(requestId, 401, 'UNAUTHORIZED', 'Unauthorized');

  const { appId } = await params;
  const tenantId = req.nextUrl.searchParams.get('tenant_id');
  if (!tenantId) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'tenant_id is required');

  const rows = await db.select().from(userProviderKeys).where(and(
    eq(userProviderKeys.app_id, appId),
    eq(userProviderKeys.tenant_id, tenantId),
    eq(userProviderKeys.user_id, authUser.id),
  ));
  return successEnvelope(requestId, { keys: rows });
}

export async function POST(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const authUser = await getUserFromAuthHeader(req.headers.get('authorization'));
  if (!authUser) return errorEnvelope(requestId, 401, 'UNAUTHORIZED', 'Unauthorized');

  const { appId } = await params;
  const body = await req.json().catch(() => null) as Record<string, unknown> | null;
  if (!body?.tenant_id || !body.provider || !body.key_label || !body.encrypted_key) {
    return errorEnvelope(
      requestId,
      400,
      'VALIDATION_ERROR',
      'tenant_id, provider, key_label and encrypted_key are required',
    );
  }

  const [row] = await db.insert(userProviderKeys).values({
    key_id: crypto.randomUUID(),
    tenant_id: String(body.tenant_id),
    app_id: appId,
    user_id: authUser.id,
    provider: String(body.provider),
    key_label: String(body.key_label),
    encrypted_key: String(body.encrypted_key),
    metadata: (body.metadata && typeof body.metadata === 'object') ? body.metadata : {},
  }).returning();

  return successEnvelope(requestId, row, 201);
}
