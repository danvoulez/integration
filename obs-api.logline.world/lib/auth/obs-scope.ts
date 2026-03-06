import type { NextRequest } from 'next/server';
import { extractBearerToken, verifySupabaseJwt, type SupabaseJwtClaims } from '@/lib/auth/supabase-jwt';

const OBS_AUTH_STRICT = process.env.OBS_AUTH_STRICT !== '0';
const DEV_SUB = process.env.DEFAULT_USER_ID || 'local-dev';

type ObsScope = 'obs:ingest' | 'obs:read' | 'obs:alerts:ack';

type ObsScopeResult =
  | {
      ok: true;
      claims: SupabaseJwtClaims;
      scopes: Set<string>;
    }
  | {
      ok: false;
      status: number;
      error: string;
    };

function normalizeScopes(claims: SupabaseJwtClaims): Set<string> {
  const items: string[] = [];

  const collect = (raw: unknown) => {
    if (typeof raw === 'string') {
      for (const part of raw.split(/[,\s]+/)) {
        const v = part.trim();
        if (v.length > 0) items.push(v);
      }
      return;
    }
    if (Array.isArray(raw)) {
      for (const v of raw) {
        if (typeof v === 'string' && v.trim().length > 0) items.push(v.trim());
      }
    }
  };

  collect(claims.scope);
  collect(claims.scp);
  collect(claims.scopes);
  collect(claims.permissions);

  return new Set(items);
}

function hasScope(scopes: Set<string>, required: ObsScope): boolean {
  return scopes.has(required) || scopes.has('*') || scopes.has('obs:*');
}

export async function requireObsScope(req: NextRequest, required: ObsScope): Promise<ObsScopeResult> {
  if (!OBS_AUTH_STRICT) {
    const scopes = new Set<string>(['obs:ingest', 'obs:read', 'obs:alerts:ack']);
    return { ok: true, claims: { sub: DEV_SUB }, scopes };
  }

  const token = extractBearerToken(req.headers.get('authorization'));
  if (!token) {
    return { ok: false, status: 401, error: 'Authorization header with Bearer token required' };
  }

  const result = await verifySupabaseJwt(token);
  if (!result.ok) {
    return { ok: false, status: 401, error: `Invalid JWT: ${result.reason}` };
  }

  const scopes = normalizeScopes(result.claims);
  if (!hasScope(scopes, required)) {
    return { ok: false, status: 403, error: `Missing required scope: ${required}` };
  }

  return { ok: true, claims: result.claims, scopes };
}
