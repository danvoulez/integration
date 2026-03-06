import { NextRequest, NextResponse } from 'next/server';
import { getRequestId } from '@/lib/api/envelope';

export async function GET(req: NextRequest): Promise<NextResponse> {
  const requestId = getRequestId(req);

  return NextResponse.json(
    {
      status: 'ok',
      request_id: requestId,
      output_schema: 'health-check.v1',
    },
    { status: 200 }
  );
}
