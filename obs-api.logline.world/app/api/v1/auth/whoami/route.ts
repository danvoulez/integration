import { NextRequest, NextResponse } from 'next/server';
import { eq } from 'drizzle-orm';
import { db } from '@/db';
import { tenantMemberships, users } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { getUserFromAuthHeader } from '@/lib/auth/supabase-server';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const authUser = await getUserFromAuthHeader(req.headers.get('authorization'));
  if (!authUser) return errorEnvelope(requestId, 401, 'UNAUTHORIZED', 'Unauthorized');

  const [user] = await db.select().from(users).where(eq(users.user_id, authUser.id));
  const memberships = await db.select().from(tenantMemberships).where(eq(tenantMemberships.user_id, authUser.id));

  return successEnvelope(requestId, {
    user: user ?? { user_id: authUser.id, email: authUser.email ?? null, display_name: authUser.user_metadata?.display_name ?? null },
    memberships,
  });
}
