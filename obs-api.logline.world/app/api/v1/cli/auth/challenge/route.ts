import { NextRequest, NextResponse } from 'next/server';
import { db } from '@/db';
import { cliAuthChallenges } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { requireJwtSubject } from '@/lib/auth/jwt-subject';
import {
  cleanupExpiredCliChallenges,
  enforceChallengeCreateRateLimit,
  loadChallengeById,
  normalizeChallengeNonce,
  normalizeDeviceName,
  resolveChallengeClientId,
  resolveChallengeExpiry,
  sanitizeCliChallenge,
} from '@/lib/auth/cli-challenge';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const body = await req.json().catch(() => null) as {
    nonce?: unknown;
    device_name?: unknown;
    expires_at?: unknown;
  } | null;
  const nonce = normalizeChallengeNonce(body?.nonce);
  if (!nonce) {
    return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'nonce is required (16..512 chars)');
  }

  await cleanupExpiredCliChallenges();

  const clientId = resolveChallengeClientId(req);
  if (!enforceChallengeCreateRateLimit(clientId)) {
    return errorEnvelope(requestId, 429, 'RATE_LIMITED', 'Too many challenge requests; retry later');
  }

  const deviceName = normalizeDeviceName(body?.device_name);
  const expiresAt = resolveChallengeExpiry(body?.expires_at);

  const challengeId = crypto.randomUUID();
  try {
    const [row] = await db.insert(cliAuthChallenges).values({
      challenge_id: challengeId,
      nonce,
      status: 'pending',
      device_name: deviceName,
      expires_at: expiresAt,
    }).returning();

    return successEnvelope(requestId, sanitizeCliChallenge(row), 201);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    if (message.toLowerCase().includes('duplicate key')) {
      return errorEnvelope(requestId, 409, 'CONFLICT', 'challenge nonce already exists');
    }
    return errorEnvelope(requestId, 500, 'INTERNAL_ERROR', 'failed to create challenge');
  }
}

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireJwtSubject(req);
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }

  const challengeId = req.nextUrl.searchParams.get('challenge_id');
  if (!challengeId) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'challenge_id is required');

  await cleanupExpiredCliChallenges();
  const row = await loadChallengeById(challengeId);
  if (!row) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Challenge not found');

  if (row.user_id && row.user_id !== auth.sub) {
    return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Challenge not found');
  }

  return successEnvelope(requestId, sanitizeCliChallenge(row));
}
