import { NextRequest, NextResponse } from 'next/server';
import { and, eq, gt } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges, tenantMemberships, users } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { requireJwtSubject } from '@/lib/auth/jwt-subject';
import {
  cleanupExpiredCliChallenges,
  loadChallengeById,
  sanitizeCliChallenge,
} from '@/lib/auth/cli-challenge';

type Params = { params: Promise<{ challengeId: string }> };

export async function POST(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireJwtSubject(req);
  if (!auth.ok) return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  const { challengeId } = await params;

  const body = await req.json().catch(() => ({})) as { tenant_id?: unknown };
  const tenantId = typeof body.tenant_id === 'string' && body.tenant_id.trim().length > 0
    ? body.tenant_id.trim().slice(0, 128)
    : null;

  await cleanupExpiredCliChallenges();

  const existing = await loadChallengeById(challengeId);
  if (!existing) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Challenge not found');

  const now = new Date();
  if (existing.expires_at.getTime() <= now.getTime()) {
    await db.update(cliAuthChallenges).set({
      status: 'expired',
      session_token: null,
    }).where(eq(cliAuthChallenges.challenge_id, challengeId));
    return errorEnvelope(requestId, 410, 'CHALLENGE_EXPIRED', 'Challenge expired');
  }

  if (existing.status !== 'pending') {
    return errorEnvelope(requestId, 409, 'CONFLICT', `Challenge cannot be approved from status '${existing.status}'`);
  }

  const challengeTenantId = tenantId ?? existing.tenant_id ?? null;
  if (challengeTenantId) {
    const memberships = await db.select().from(tenantMemberships).where(and(
      eq(tenantMemberships.tenant_id, challengeTenantId),
      eq(tenantMemberships.user_id, auth.sub),
    )).limit(1);
    if (memberships.length === 0) {
      return errorEnvelope(requestId, 403, 'FORBIDDEN', 'User is not a member of this tenant');
    }
  }

  await db.insert(users).values({
    user_id: auth.sub,
    display_name: auth.sub,
    created_at: new Date(),
  }).onConflictDoNothing();

  const [updated] = await db.update(cliAuthChallenges).set({
    status: 'approved',
    user_id: auth.sub,
    tenant_id: challengeTenantId,
    approved_at: now,
    session_token: crypto.randomUUID(),
  }).where(and(
    eq(cliAuthChallenges.challenge_id, challengeId),
    eq(cliAuthChallenges.status, 'pending'),
    gt(cliAuthChallenges.expires_at, now),
  )).returning();

  if (!updated) {
    return errorEnvelope(requestId, 409, 'CONFLICT', 'Challenge approval raced with another action');
  }

  return successEnvelope(requestId, sanitizeCliChallenge(updated));
}
