import { NextRequest, NextResponse } from 'next/server';
import { and, eq } from 'drizzle-orm';
import { db } from '@/db';
import { tenantMemberships, tenants } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';
import { requireJwtSubject } from '@/lib/auth/jwt-subject';

const TENANT_RESOLVE_WINDOW_MS = 60_000;
const TENANT_RESOLVE_MAX_REQUESTS = 30;
const tenantResolveBuckets = new Map<string, { count: number; windowStartMs: number }>();

function allowTenantResolveAttempt(userId: string, now = Date.now()): boolean {
  for (const [key, bucket] of tenantResolveBuckets.entries()) {
    if ((now - bucket.windowStartMs) >= TENANT_RESOLVE_WINDOW_MS) {
      tenantResolveBuckets.delete(key);
    }
  }

  const current = tenantResolveBuckets.get(userId);
  if (!current) {
    tenantResolveBuckets.set(userId, { count: 1, windowStartMs: now });
    return true;
  }

  if ((now - current.windowStartMs) >= TENANT_RESOLVE_WINDOW_MS) {
    tenantResolveBuckets.set(userId, { count: 1, windowStartMs: now });
    return true;
  }

  current.count += 1;
  tenantResolveBuckets.set(userId, current);
  return current.count <= TENANT_RESOLVE_MAX_REQUESTS;
}

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const auth = await requireJwtSubject(req);
  if (!auth.ok) {
    return errorEnvelope(requestId, auth.status, 'UNAUTHORIZED', auth.error);
  }
  if (!allowTenantResolveAttempt(auth.sub)) {
    return errorEnvelope(requestId, 429, 'RATE_LIMITED', 'Too many tenant resolve requests');
  }

  const body = await req.json().catch(() => null) as { slug?: string } | null;
  if (!body?.slug) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'slug is required');
  const slug = body.slug.trim();
  if (!slug) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'slug is required');

  const [tenant] = await db.select().from(tenants).where(eq(tenants.slug, slug)).limit(1);
  if (!tenant) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Tenant not found');

  const [membership] = await db.select().from(tenantMemberships).where(and(
    eq(tenantMemberships.tenant_id, tenant.tenant_id),
    eq(tenantMemberships.user_id, auth.sub),
  )).limit(1);
  if (!membership) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Tenant not found');

  return successEnvelope(requestId, {
    tenant_id: tenant.tenant_id,
    slug: tenant.slug,
    name: tenant.name,
  });
}
