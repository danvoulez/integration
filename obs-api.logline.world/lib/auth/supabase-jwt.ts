import {
  createRemoteJWKSet,
  decodeProtectedHeader,
  importJWK,
  jwtVerify,
  type JWK,
} from 'jose';

const AUTH_PROVIDER_MODE = process.env.AUTH_PROVIDER_MODE ?? 'jwt';
const SUPABASE_JWT_SECRET = process.env.SUPABASE_JWT_SECRET ?? '';
const SUPABASE_JWT_PUBLIC_JWK = process.env.SUPABASE_JWT_PUBLIC_JWK ?? '';
const SUPABASE_JWT_DISCOVERY_URL = process.env.SUPABASE_JWT_DISCOVERY_URL ?? '';
const SUPABASE_JWKS_URL = process.env.SUPABASE_JWKS_URL ?? SUPABASE_JWT_DISCOVERY_URL;
const SUPABASE_JWT_ISSUER = process.env.SUPABASE_JWT_ISSUER;
const SUPABASE_JWT_AUDIENCE = process.env.SUPABASE_JWT_AUDIENCE;

let remoteJwks: ReturnType<typeof createRemoteJWKSet> | null = null;

export type SupabaseJwtClaims = {
  sub: string;
  email?: string;
  role?: string;
  aal?: string;
  session_id?: string;
  workspace_id?: string;
  app_id?: string;
  scope?: string;
  scp?: string | string[];
  scopes?: string | string[];
  permissions?: string[];
  iss?: string;
  exp?: number;
  iat?: number;
};

export type JwtResult =
  | { ok: true; claims: SupabaseJwtClaims }
  | { ok: false; reason: string };

export function isCompatMode(): boolean {
  return AUTH_PROVIDER_MODE === 'compat';
}

function verifyOptions(alg: string): {
  algorithms: string[];
  issuer?: string;
  audience?: string;
} {
  return {
    algorithms: [alg],
    issuer: SUPABASE_JWT_ISSUER || undefined,
    audience: SUPABASE_JWT_AUDIENCE || undefined,
  };
}

function getJwksVerifier():
  | { ok: true; verifyKey: ReturnType<typeof createRemoteJWKSet> }
  | { ok: false; reason: string } {
  if (!SUPABASE_JWKS_URL) {
    return { ok: false, reason: 'SUPABASE_JWKS_URL/SUPABASE_JWT_DISCOVERY_URL is not configured' };
  }
  try {
    if (!remoteJwks) {
      remoteJwks = createRemoteJWKSet(new URL(SUPABASE_JWKS_URL));
    }
    return { ok: true, verifyKey: remoteJwks };
  } catch {
    return { ok: false, reason: `Invalid JWKS URL: ${SUPABASE_JWKS_URL}` };
  }
}

async function verifyWithSecret(token: string, alg: string): Promise<JwtResult> {
  if (!SUPABASE_JWT_SECRET) {
    return { ok: false, reason: 'SUPABASE_JWT_SECRET is not configured' };
  }
  try {
    const secret = new TextEncoder().encode(SUPABASE_JWT_SECRET);
    const { payload } = await jwtVerify(token, secret, verifyOptions(alg));
    const sub = payload.sub;
    if (!sub) {
      return { ok: false, reason: 'JWT missing sub claim' };
    }
    return {
      ok: true,
      claims: payload as unknown as SupabaseJwtClaims,
    };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, reason: `JWT verification failed: ${message}` };
  }
}

async function verifyWithAsymmetric(token: string, alg: string): Promise<JwtResult> {
  try {
    const keySource = SUPABASE_JWT_PUBLIC_JWK.trim();
    const { payload } = await (keySource
      ? jwtVerify(token, await importJWK(JSON.parse(keySource) as JWK, alg), verifyOptions(alg))
      : (() => {
          const jwks = getJwksVerifier();
          if (!jwks.ok) {
            throw new Error(jwks.reason);
          }
          return jwtVerify(token, jwks.verifyKey, verifyOptions(alg));
        })());

    const sub = payload.sub;
    if (!sub) {
      return { ok: false, reason: 'JWT missing sub claim' };
    }
    return {
      ok: true,
      claims: payload as unknown as SupabaseJwtClaims,
    };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, reason: `JWT verification failed: ${message}` };
  }
}

/**
 * Extracts a Bearer token from the Authorization header.
 * Returns null if the header is absent or malformed.
 */
export function extractBearerToken(authHeader: string | null): string | null {
  if (!authHeader) return null;
  const parts = authHeader.split(' ');
  if (parts.length !== 2 || parts[0].toLowerCase() !== 'bearer') return null;
  const token = parts[1].trim();
  return token.length > 0 ? token : null;
}

/**
 * Verifies a Supabase JWT (HS256) using SUPABASE_JWT_SECRET.
 * Returns the claims on success, or an error reason on failure.
 */
export async function verifySupabaseJwt(token: string): Promise<JwtResult> {
  let alg = '';
  try {
    alg = decodeProtectedHeader(token).alg ?? '';
  } catch {
    return { ok: false, reason: 'JWT header parse failed' };
  }

  if (!alg) {
    return { ok: false, reason: 'JWT header missing alg' };
  }

  if (alg.startsWith('HS')) {
    return verifyWithSecret(token, alg);
  }

  if (alg.startsWith('ES') || alg.startsWith('RS') || alg.startsWith('PS')) {
    return verifyWithAsymmetric(token, alg);
  }

  return { ok: false, reason: `Unsupported JWT alg: ${alg}` };
}
