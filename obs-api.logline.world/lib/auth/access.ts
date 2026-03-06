import type { NextRequest } from 'next/server';
import { and, eq, sql as drizzleSql } from 'drizzle-orm';
import { db } from '@/db/index';
import { appMemberships, apps, tenantMemberships, tenants, users } from '@/db/schema';
import { resolveWorkspaceId } from '@/lib/auth/workspace';
import {
  extractBearerToken,
  isCompatMode,
  verifySupabaseJwt,
} from '@/lib/auth/supabase-jwt';

export type TenantRole = 'member' | 'admin';
export type AppRole = 'member' | 'app_admin';
export type AccessPermission = 'read' | 'write' | 'private_read';

const DEFAULT_APP_ID = process.env.DEFAULT_APP_ID || 'ublx';
const DEFAULT_USER_ID = process.env.DEFAULT_USER_ID || 'local-dev';
const RBAC_STRICT = process.env.RBAC_STRICT !== '0';

export class AccessDeniedError extends Error {
  status: number;
  constructor(message: string, status = 403) {
    super(message);
    this.status = status;
  }
}

export type AccessContext = {
  tenantId: string;
  workspaceId: string;
  appId: string;
  userId: string;
  tenantRole: TenantRole;
  appRole: AppRole;
};

function resolveAppId(req: NextRequest): string {
  const fromHeader = req.headers.get('x-app-id')?.trim();
  if (fromHeader) return fromHeader;
  const fromQuery = req.nextUrl.searchParams.get('app_id')?.trim();
  if (fromQuery) return fromQuery;
  return DEFAULT_APP_ID;
}

/**
 * Resolve userId from:
 *  1. Supabase JWT `sub` from Authorization header (JWT mode, default)
 *  2. x-user-id header or user_id query param (compat mode only)
 *
 * Returns { userId, fromJwt, jwtWorkspaceId, jwtAppId } so callers can
 * optionally override workspace/app from token claims.
 */
async function resolveIdentity(req: NextRequest): Promise<{
  userId: string;
  fromJwt: boolean;
  jwtWorkspaceId?: string;
  jwtAppId?: string;
}> {
  // In compat mode, fall through to header-based identity directly.
  if (!isCompatMode()) {
    const authHeader = req.headers.get('authorization');
    const token = extractBearerToken(authHeader);
    if (token) {
      const result = await verifySupabaseJwt(token);
      if (result.ok) {
        return {
          userId: result.claims.sub,
          fromJwt: true,
          jwtWorkspaceId: result.claims.workspace_id,
          jwtAppId: result.claims.app_id,
        };
      }
      // If JWT is present but invalid, fail hard in strict mode.
      if (RBAC_STRICT) {
        throw new AccessDeniedError(`Invalid JWT: ${result.reason}`, 401);
      }
    } else if (RBAC_STRICT) {
      throw new AccessDeniedError('Authorization header with Bearer token required', 401);
    }
  }

  // Header/query fallback (compat mode or non-strict dev mode).
  const fromHeader = req.headers.get('x-user-id')?.trim();
  if (fromHeader) return { userId: fromHeader, fromJwt: false };
  const fromQuery = req.nextUrl.searchParams.get('user_id')?.trim();
  if (fromQuery) return { userId: fromQuery, fromJwt: false };
  return { userId: DEFAULT_USER_ID, fromJwt: false };
}

async function ensureLocalDevBootstrap(tenantId: string, appId: string, userId: string): Promise<void> {
  if (RBAC_STRICT) return;
  if (userId !== DEFAULT_USER_ID) return;

  await db
    .insert(users)
    .values({
      user_id: userId,
      display_name: 'Local Dev',
      created_at: new Date(),
    })
    .onConflictDoNothing();

  await db
    .insert(tenants)
    .values({
      tenant_id: tenantId,
      slug: tenantId,
      name: tenantId,
      created_at: new Date(),
    })
    .onConflictDoNothing();

  await db
    .insert(apps)
    .values({
      app_id: appId,
      tenant_id: tenantId,
      name: appId,
      created_at: new Date(),
    })
    .onConflictDoNothing();

  await db
    .insert(tenantMemberships)
    .values({
      tenant_id: tenantId,
      user_id: userId,
      role: 'admin',
      created_at: new Date(),
    })
    .onConflictDoNothing();

  await db
    .insert(appMemberships)
    .values({
      app_id: appId,
      tenant_id: tenantId,
      user_id: userId,
      role: 'app_admin',
      created_at: new Date(),
    })
    .onConflictDoNothing();
}

export async function requireAccess(
  req: NextRequest,
  permission: AccessPermission
): Promise<AccessContext> {
  const { userId, fromJwt, jwtWorkspaceId, jwtAppId } = await resolveIdentity(req);

  // Workspace/tenant: prefer JWT claim, then header/query, then default.
  let workspaceId = resolveWorkspaceId(req);
  if (fromJwt && jwtWorkspaceId && workspaceId === (process.env.DEFAULT_WORKSPACE_ID || 'default')) {
    workspaceId = jwtWorkspaceId;
  }
  const tenantId = workspaceId;

  // App: prefer JWT claim, then header/query, then default.
  let appId = resolveAppId(req);
  if (fromJwt && jwtAppId && appId === DEFAULT_APP_ID) {
    appId = jwtAppId;
  }

  await ensureLocalDevBootstrap(tenantId, appId, userId);

  const membershipCount = await db
    .select({ count: drizzleSql<number>`count(*)::int` })
    .from(tenantMemberships)
    .where(eq(tenantMemberships.user_id, userId));
  const totalTenantMemberships = membershipCount[0]?.count ?? 0;
  if (totalTenantMemberships <= 0) {
    throw new AccessDeniedError('User must belong to at least one tenant', 403);
  }

  const tenantRows = await db
    .select({ role: tenantMemberships.role })
    .from(tenantMemberships)
    .where(and(eq(tenantMemberships.tenant_id, tenantId), eq(tenantMemberships.user_id, userId)))
    .limit(1);
  if (tenantRows.length === 0) {
    throw new AccessDeniedError('User is not a member of this tenant', 403);
  }

  const appRows = await db
    .select({ role: appMemberships.role })
    .from(appMemberships)
    .where(
      and(
        eq(appMemberships.app_id, appId),
        eq(appMemberships.tenant_id, tenantId),
        eq(appMemberships.user_id, userId)
      )
    )
    .limit(1);
  if (appRows.length === 0) {
    throw new AccessDeniedError('User is not a member of this app', 403);
  }

  const tenantRole = (tenantRows[0].role === 'admin' ? 'admin' : 'member') as TenantRole;
  const appRole = (appRows[0].role === 'app_admin' ? 'app_admin' : 'member') as AppRole;

  const canWrite = appRole === 'app_admin';
  const canReadPrivate = appRole === 'app_admin';

  if (permission === 'write' && !canWrite) {
    throw new AccessDeniedError('Members cannot change tenant/app data', 403);
  }

  if (permission === 'private_read' && !canReadPrivate) {
    throw new AccessDeniedError('Private data is only available to app admins', 403);
  }

  return {
    tenantId,
    workspaceId,
    appId,
    userId,
    tenantRole,
    appRole,
  };
}
