import { NextRequest, NextResponse } from 'next/server';
import { and, eq } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import {
  cleanupExpiredCliChallenges,
  normalizeChallengeNonce,
  sanitizeCliChallengeStatus,
} from '@/lib/auth/cli-challenge';

type Params = { params: Promise<{ challengeId: string }> };

export async function GET(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const { challengeId } = await params;
  const nonce = normalizeChallengeNonce(req.nextUrl.searchParams.get('nonce'));
  if (!nonce) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'nonce is required');
  }

  await cleanupExpiredCliChallenges();
  const rows = await db.select().from(cliAuthChallenges).where(and(
    eq(cliAuthChallenges.challenge_id, challengeId),
    eq(cliAuthChallenges.nonce, nonce),
  )).limit(1);

  const row = rows[0];
  if (!row) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Challenge not found');

  const now = Date.now();
  const isExpired = row.expires_at.getTime() <= now;
  if (isExpired && row.status !== 'expired') {
    await db.update(cliAuthChallenges).set({
      status: 'expired',
      session_token: null,
    }).where(eq(cliAuthChallenges.challenge_id, challengeId));
    return successEnvelope(
      requestId,
      sanitizeCliChallengeStatus({ ...row, status: 'expired', session_token: null }),
    );
  }

  let oneTimeSessionToken: string | null = null;
  if (row.status === 'approved' && row.session_token) {
    const consumedRows = await db.update(cliAuthChallenges).set({
      session_token: null,
    }).where(and(
      eq(cliAuthChallenges.challenge_id, challengeId),
      eq(cliAuthChallenges.nonce, nonce),
      eq(cliAuthChallenges.status, 'approved'),
      eq(cliAuthChallenges.session_token, row.session_token),
    )).returning({ challenge_id: cliAuthChallenges.challenge_id });
    if (consumedRows.length > 0) {
      oneTimeSessionToken = row.session_token;
    }
  }

  return successEnvelope(requestId, sanitizeCliChallengeStatus(row, oneTimeSessionToken));
}
