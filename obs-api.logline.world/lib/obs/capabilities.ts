import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { z } from 'zod';

export const capabilitiesCatalogQuerySchema = z.object({
  service_id: z.string().trim().min(1).max(128).optional(),
  surface: z.enum(['all', 'cli', 'api']).default('all'),
});

export type CapabilitiesCatalogQuery = z.infer<typeof capabilitiesCatalogQuerySchema>;

function resolveCatalogPath(): string {
  const configured = process.env.CAPABILITY_CATALOG_PATH;
  if (configured && configured.trim().length > 0) {
    return path.isAbsolute(configured)
      ? configured
      : path.resolve(process.cwd(), configured);
  }

  return path.resolve(process.cwd(), '..', 'contracts', 'generated', 'capability-catalog.v1.json');
}

export async function getCapabilitiesCatalog(query: CapabilitiesCatalogQuery) {
  const catalogPath = resolveCatalogPath();
  const raw = await readFile(catalogPath, 'utf8');
  const parsed = JSON.parse(raw) as {
    catalog_version?: string;
    generated_at?: string;
    summary?: Record<string, unknown>;
    capabilities?: Array<Record<string, unknown>>;
    sources?: Record<string, unknown>;
  };

  const capabilities = Array.isArray(parsed.capabilities) ? parsed.capabilities : [];
  const filteredCapabilities = capabilities.filter((item) => {
    const surface = String(item.surface ?? '');
    if (query.surface !== 'all' && surface !== query.surface) {
      return false;
    }

    if (!query.service_id) {
      return true;
    }

    const serviceId = String(item.service_id ?? item.service ?? '');
    return serviceId === query.service_id;
  });

  return {
    catalog_version: parsed.catalog_version ?? 'logline.capability-catalog.v1',
    generated_at: parsed.generated_at ?? null,
    source_path: catalogPath,
    filters: {
      service_id: query.service_id ?? null,
      surface: query.surface,
    },
    totals: {
      capabilities: filteredCapabilities.length,
    },
    summary: parsed.summary ?? {},
    sources: parsed.sources ?? {},
    capabilities: filteredCapabilities,
  };
}
