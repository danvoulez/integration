import { NextRequest, NextResponse } from 'next/server';
import { and, eq } from 'drizzle-orm';
import { db } from '@/db';
import { appMemberships, userProviderKeys, users } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { requireJwtSubject } from '@/lib/auth/jwt-subject';

type Params = { params: Promise<{ appId: string }> };

async function hasAppMembership(userId: string, tenantId: string, appId: string): Promise<boolean> {
  const rows = await db.select({ role: appMemberships.role }).from(appMemberships).where(and(
    eq(appMemberships.app_id, appId),
    eq(appMemberships.tenant_id, tenantId),
    eq(appMemberships.user_id, userId),
  )).limit(1);
  return rows.length > 0;
}

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireJwtSubject(req);
  if (!auth.ok) return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);

  const { appId } = await params;
  const tenantId = req.nextUrl.searchParams.get('tenant_id');
  if (!tenantId) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'tenant_id is required');
  if (!(await hasAppMembership(auth.sub, tenantId, appId))) {
    return errorEnvelope(requestId, 403, 'FORBIDDEN', 'User is not a member of this tenant/app');
  }

  const rows = await db.select().from(userProviderKeys).where(and(
    eq(userProviderKeys.app_id, appId),
    eq(userProviderKeys.tenant_id, tenantId),
    eq(userProviderKeys.user_id, auth.sub),
  ));
  return successEnvelope(requestId, { keys: rows });
}

export async function POST(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireJwtSubject(req);
  if (!auth.ok) return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);

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
  const tenantId = String(body.tenant_id);
  if (!(await hasAppMembership(auth.sub, tenantId, appId))) {
    return errorEnvelope(requestId, 403, 'FORBIDDEN', 'User is not a member of this tenant/app');
  }

  await db.insert(users).values({
    user_id: auth.sub,
    display_name: auth.sub,
    created_at: new Date(),
  }).onConflictDoNothing();

  const [row] = await db.insert(userProviderKeys).values({
    key_id: crypto.randomUUID(),
    tenant_id: tenantId,
    app_id: appId,
    user_id: auth.sub,
    provider: String(body.provider),
    key_label: String(body.key_label),
    encrypted_key: String(body.encrypted_key),
    metadata: (body.metadata && typeof body.metadata === 'object') ? body.metadata : {},
  }).returning();

  return successEnvelope(requestId, row, 201);
}
