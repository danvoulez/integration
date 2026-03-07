import { NextRequest, NextResponse } from 'next/server';
import { eq } from 'drizzle-orm';
import { db } from '@/db';
import { cliAuthChallenges } from '@/db/schema';

type Params = { params: Promise<{ challengeId: string }> };

export async function GET(_req: NextRequest, { params }: Params): Promise<NextResponse> {
  const { challengeId } = await params;
  const [row] = await db.select().from(cliAuthChallenges).where(eq(cliAuthChallenges.challenge_id, challengeId));
  if (!row) return NextResponse.json({ error: 'Challenge not found' }, { status: 404 });
  return NextResponse.json(row);
}
