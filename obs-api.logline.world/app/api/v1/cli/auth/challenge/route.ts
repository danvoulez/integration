import { NextRequest, NextResponse } from 'next/server';
import { eq } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges } from '@/db/schema';

export async function POST(req: NextRequest): Promise<NextResponse> {
  const body = await req.json().catch(() => null) as { nonce?: string; device_name?: string; expires_at?: string } | null;
  if (!body?.nonce) return NextResponse.json({ error: 'nonce is required' }, { status: 400 });

  const challengeId = crypto.randomUUID();
  const [row] = await db.insert(cliAuthChallenges).values({
    challenge_id: challengeId,
    nonce: body.nonce,
    status: 'pending',
    device_name: body.device_name ?? null,
    expires_at: body.expires_at ? new Date(body.expires_at) : new Date(Date.now() + 5 * 60_000),
  }).returning();

  return NextResponse.json(row, { status: 201 });
}

export async function GET(req: NextRequest): Promise<NextResponse> {
  const challengeId = req.nextUrl.searchParams.get('challenge_id');
  if (!challengeId) return NextResponse.json({ error: 'challenge_id is required' }, { status: 400 });
  const [row] = await db.select().from(cliAuthChallenges).where(eq(cliAuthChallenges.challenge_id, challengeId));
  if (!row) return NextResponse.json({ error: 'Challenge not found' }, { status: 404 });
  return NextResponse.json(row);
}
