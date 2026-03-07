import { NextRequest, NextResponse } from 'next/server';
import { eq } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges } from '@/db/schema';
import { getUserFromAuthHeader } from '@/lib/auth/supabase-server';

type Params = { params: Promise<{ challengeId: string }> };

export async function POST(req: NextRequest, { params }: Params): Promise<NextResponse> {
  const user = await getUserFromAuthHeader(req.headers.get('authorization'));
  if (!user) return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  const { challengeId } = await params;

  const body = await req.json().catch(() => ({})) as { tenant_id?: string };

  const [updated] = await db.update(cliAuthChallenges).set({
    status: 'approved',
    user_id: user.id,
    tenant_id: body.tenant_id ?? null,
    approved_at: new Date(),
    session_token: crypto.randomUUID(),
  }).where(eq(cliAuthChallenges.challenge_id, challengeId)).returning();

  if (!updated) return NextResponse.json({ error: 'Challenge not found' }, { status: 404 });
  return NextResponse.json(updated);
}
