import { NextRequest, NextResponse } from 'next/server';
import { eq } from 'drizzle-orm';
import { db } from '@/db';
import { tenants } from '@/db/schema';
import { errorEnvelope, getRequestId, successEnvelope } from '@/lib/api/envelope';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);
  const body = await req.json().catch(() => null) as { slug?: string } | null;
  if (!body?.slug) return errorEnvelope(requestId, 400, 'VALIDATION_ERROR', 'slug is required');
  const [tenant] = await db.select().from(tenants).where(eq(tenants.slug, body.slug));
  if (!tenant) return errorEnvelope(requestId, 404, 'NOT_FOUND', 'Tenant not found');
  return successEnvelope(requestId, tenant);
}
