import { NextRequest, NextResponse } from 'next/server';

const RESPONSE_ENVELOPE_SCHEMA = 'https://logline.world/schemas/response-envelope.v1.schema.json';
const ERROR_ENVELOPE_SCHEMA = 'https://logline.world/schemas/error-envelope.v1.schema.json';

export function getRequestId(req: NextRequest): string {
  const requestId = req.headers.get('x-request-id')?.trim();
  if (requestId && requestId.length > 0) return requestId;
  return crypto.randomUUID();
}

export function successEnvelope(
  requestId: string,
  payload: unknown,
  status = 200,
  outputSchema = RESPONSE_ENVELOPE_SCHEMA,
): NextResponse {
  const body = toObjectPayload(payload);
  body.request_id = requestId;
  body.output_schema = outputSchema;

  const response = NextResponse.json(body, { status });
  response.headers.set('x-request-id', requestId);
  return response;
}

export function errorEnvelope(
  requestId: string,
  status: number,
  type: string,
  message: string,
  details?: unknown,
): NextResponse {
  const response = NextResponse.json(
    {
      request_id: requestId,
      output_schema: ERROR_ENVELOPE_SCHEMA,
      error: {
        type,
        code: type,
        message,
        details: details ?? {},
      },
    },
    { status },
  );
  response.headers.set('x-request-id', requestId);
  return response;
}

function toObjectPayload(payload: unknown): Record<string, unknown> {
  if (payload !== null && typeof payload === 'object' && !Array.isArray(payload)) {
    return { ...(payload as Record<string, unknown>) };
  }
  return { data: payload };
}
