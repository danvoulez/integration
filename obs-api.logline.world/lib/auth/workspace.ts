import type { NextRequest } from 'next/server';

export const DEFAULT_WORKSPACE_ID = process.env.DEFAULT_WORKSPACE_ID || 'default';

export function resolveWorkspaceId(req: NextRequest): string {
  const fromHeader = req.headers.get('x-workspace-id')?.trim();
  if (fromHeader) return fromHeader;

  const fromQuery = req.nextUrl.searchParams.get('workspace_id')?.trim();
  if (fromQuery) return fromQuery;

  return DEFAULT_WORKSPACE_ID;
}

export function toScopedKey(workspaceId: string, key: string): string {
  return `ws:${workspaceId}:${key}`;
}

export function toScopedAppKey(workspaceId: string, appId: string, key: string): string {
  return `ws:${workspaceId}:app:${appId}:${key}`;
}
