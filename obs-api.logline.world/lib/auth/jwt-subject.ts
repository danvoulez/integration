import type { NextRequest } from 'next/server';
import { extractBearerToken, verifySupabaseJwt, type SupabaseJwtClaims } from '@/lib/auth/supabase-jwt';

export type JwtSubjectResult =
  | { ok: true; sub: string; claims: SupabaseJwtClaims }
  | { ok: false; status: number; error: string };

export async function requireJwtSubject(req: NextRequest): Promise<JwtSubjectResult> {
  const token = extractBearerToken(req.headers.get('authorization'));
  if (!token) {
    return { ok: false, status: 401, error: 'Authorization header with Bearer token required' };
  }

  const result = await verifySupabaseJwt(token);
  if (!result.ok) {
    return { ok: false, status: 401, error: `Invalid JWT: ${result.reason}` };
  }

  return { ok: true, sub: result.claims.sub, claims: result.claims };
}
