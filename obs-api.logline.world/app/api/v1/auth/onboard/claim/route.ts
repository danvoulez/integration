import { NextRequest, NextResponse } from 'next/server';
import { and, eq, isNull, or, sql } from 'drizzle-orm';
import { db } from '@/db';
import { tenantEmailAllowlist, tenantMemberships, users } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getUserFromAuthHeader } from '@/lib/auth/supabase-server';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const authUser = await getUserFromAuthHeader(req.headers.get('authorization'));
  if (!authUser?.email) return errorEnvelope(requestId, 401, 'UNAUTHORIZED', 'Unauthorized');

  const email = authUser.email.trim().toLowerCase();
  await db.insert(users).values({ user_id: authUser.id, email: authUser.email }).onConflictDoNothing();

  const allowlistRows = await db.select().from(tenantEmailAllowlist)
    .where(and(eq(tenantEmailAllowlist.email_normalized, email), or(isNull(tenantEmailAllowlist.expires_at), sql`${tenantEmailAllowlist.expires_at} > now()`)));

  for (const row of allowlistRows) {
    await db.insert(tenantMemberships).values({ tenant_id: row.tenant_id, user_id: authUser.id, role: row.role_default }).onConflictDoNothing();
  }

  const memberships = await db.select().from(tenantMemberships).where(eq(tenantMemberships.user_id, authUser.id));
  return successEnvelope(requestId, { ok: true, memberships });
}
