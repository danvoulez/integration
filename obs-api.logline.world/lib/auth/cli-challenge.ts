import type { NextRequest } from 'next/server';
import { and, eq, inArray, lt } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges, type CliAuthChallenge } from '@/db/schema';

const DEFAULT_CHALLENGE_TTL_MS = 5 * 60_000;
const MIN_CHALLENGE_TTL_MS = 60_000;
const MAX_CHALLENGE_TTL_MS = 10 * 60_000;
const CHALLENGE_RETENTION_MS = 24 * 60 * 60_000;

const RATE_LIMIT_WINDOW_MS = 10 * 60_000;
const RATE_LIMIT_MAX_REQUESTS = 15;

type RateLimitBucket = {
  count: number;
  windowStartedAtMs: number;
};

const challengeBuckets = new Map<string, RateLimitBucket>();

function trimTo(value: string | null | undefined, max: number): string | null {
  if (typeof value !== 'string') return null;
  const normalized = value.trim();
  if (!normalized) return null;
  return normalized.slice(0, max);
}

export function normalizeChallengeNonce(input: unknown): string | null {
  if (typeof input !== 'string') return null;
  const nonce = input.trim();
  if (nonce.length < 16 || nonce.length > 512) return null;
  return nonce;
}

export function normalizeDeviceName(input: unknown): string | null {
  if (typeof input !== 'string') return null;
  return trimTo(input, 128);
}

export function resolveChallengeExpiry(input: unknown, now = new Date()): Date {
  const fallback = new Date(now.getTime() + DEFAULT_CHALLENGE_TTL_MS);

  if (typeof input !== 'string' || input.trim().length === 0) {
    return fallback;
  }

  const parsed = new Date(input);
  if (Number.isNaN(parsed.getTime())) {
    return fallback;
  }

  const minExpiry = new Date(now.getTime() + MIN_CHALLENGE_TTL_MS);
  const maxExpiry = new Date(now.getTime() + MAX_CHALLENGE_TTL_MS);
  if (parsed.getTime() < minExpiry.getTime()) return minExpiry;
  if (parsed.getTime() > maxExpiry.getTime()) return maxExpiry;
  return parsed;
}

export function resolveChallengeClientId(req: NextRequest): string {
  const forwardedFor = req.headers.get('x-forwarded-for');
  const ip = trimTo(forwardedFor?.split(',')[0] ?? req.headers.get('x-real-ip'), 64) ?? 'unknown-ip';
  const ua = trimTo(req.headers.get('user-agent'), 128) ?? 'unknown-ua';
  return `${ip}|${ua}`;
}

export function enforceChallengeCreateRateLimit(clientId: string, now = Date.now()): boolean {
  for (const [key, bucket] of challengeBuckets.entries()) {
    if ((now - bucket.windowStartedAtMs) >= RATE_LIMIT_WINDOW_MS) {
      challengeBuckets.delete(key);
    }
  }

  const current = challengeBuckets.get(clientId);
  if (!current) {
    challengeBuckets.set(clientId, { count: 1, windowStartedAtMs: now });
    return true;
  }

  if ((now - current.windowStartedAtMs) >= RATE_LIMIT_WINDOW_MS) {
    challengeBuckets.set(clientId, { count: 1, windowStartedAtMs: now });
    return true;
  }

  current.count += 1;
  challengeBuckets.set(clientId, current);
  return current.count <= RATE_LIMIT_MAX_REQUESTS;
}

export async function cleanupExpiredCliChallenges(now = new Date()): Promise<{ expired: number; pruned: number }> {
  const expiredRows = await db
    .update(cliAuthChallenges)
    .set({
      status: 'expired',
      session_token: null,
    })
    .where(and(
      inArray(cliAuthChallenges.status, ['pending', 'approved']),
      lt(cliAuthChallenges.expires_at, now),
    ))
    .returning({ challenge_id: cliAuthChallenges.challenge_id });

  const pruneBefore = new Date(now.getTime() - CHALLENGE_RETENTION_MS);
  const prunedRows = await db
    .delete(cliAuthChallenges)
    .where(and(
      inArray(cliAuthChallenges.status, ['expired', 'denied']),
      lt(cliAuthChallenges.created_at, pruneBefore),
    ))
    .returning({ challenge_id: cliAuthChallenges.challenge_id });

  return {
    expired: expiredRows.length,
    pruned: prunedRows.length,
  };
}

export function sanitizeCliChallenge(row: CliAuthChallenge) {
  return {
    challenge_id: row.challenge_id,
    status: row.status,
    device_name: row.device_name,
    tenant_id: row.tenant_id,
    expires_at: row.expires_at,
    approved_at: row.approved_at,
    created_at: row.created_at,
  };
}

export function sanitizeCliChallengeStatus(row: CliAuthChallenge, sessionToken: string | null = null) {
  return {
    challenge_id: row.challenge_id,
    status: row.status,
    device_name: row.device_name,
    tenant_id: row.tenant_id,
    expires_at: row.expires_at,
    approved_at: row.approved_at,
    poll_after_ms: row.status === 'pending' ? 2_000 : 0,
    ...(sessionToken ? { session_token: sessionToken } : {}),
  };
}

export async function loadChallengeById(challengeId: string): Promise<CliAuthChallenge | null> {
  const rows = await db
    .select()
    .from(cliAuthChallenges)
    .where(eq(cliAuthChallenges.challenge_id, challengeId))
    .limit(1);
  return rows[0] ?? null;
}
