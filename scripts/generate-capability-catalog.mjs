#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, stat, writeFile, mkdir } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const outputPath = process.argv[2]
  ? path.resolve(process.cwd(), process.argv[2])
  : path.join(rootDir, 'contracts', 'generated', 'capability-catalog.v1.json');

const cliWorkspace = path.join(rootDir, 'logic.logline.world');
const apiSpecs = [
  { service_id: 'obs-api', file: 'obs-api.logline.world/openapi.yaml', surface: 'public' },
  { service_id: 'code247', file: 'code247.logline.world/openapi.yaml', surface: 'public' },
  { service_id: 'llm-gateway', file: 'llm-gateway.logline.world/openapi.yaml', surface: 'public' },
  { service_id: 'edge-control', file: 'contracts/openapi/edge-control.v1.openapi.yaml', surface: 'public' },
  { service_id: 'inference-plane', file: 'contracts/openapi/inference-plane.v1.openapi.yaml', surface: 'internal' },
];

function stripQuotes(value) {
  const trimmed = value.trim();
  if ((trimmed.startsWith('"') && trimmed.endsWith('"')) || (trimmed.startsWith("'") && trimmed.endsWith("'"))) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function slugPath(rawPath) {
  return rawPath
    .replace(/^\/+/, '')
    .replace(/[{}]/g, '')
    .replace(/\//g, '.')
    .replace(/[^a-zA-Z0-9.]/g, '-')
    .replace(/\.+/g, '.')
    .replace(/\.$/, '')
    .toLowerCase();
}

function parseOpenApiYaml(content) {
  const title = stripQuotes(content.match(/^  title:\s*(.+)$/m)?.[1] ?? 'unknown');
  const version = stripQuotes(content.match(/^  version:\s*(.+)$/m)?.[1] ?? 'unknown');
  const servers = [...content.matchAll(/^  - url:\s*(.+)$/gm)].map((match) => stripQuotes(match[1]));

  const endpoints = [];
  const lines = content.split(/\r?\n/);
  let inPaths = false;
  let currentPath = null;
  let currentEndpoint = null;

  for (const line of lines) {
    if (/^paths:\s*$/.test(line)) {
      inPaths = true;
      currentPath = null;
      currentEndpoint = null;
      continue;
    }

    if (inPaths && /^[^\s]/.test(line)) {
      inPaths = false;
      currentPath = null;
      currentEndpoint = null;
    }

    if (!inPaths) continue;

    const pathMatch = line.match(/^  (\/[^:]+):\s*$/);
    if (pathMatch) {
      currentPath = pathMatch[1];
      currentEndpoint = null;
      continue;
    }

    const methodMatch = line.match(/^    (get|post|put|patch|delete|options|head):\s*$/);
    if (methodMatch && currentPath) {
      currentEndpoint = {
        method: methodMatch[1].toUpperCase(),
        path: currentPath,
        summary: null,
        tags: [],
      };
      endpoints.push(currentEndpoint);
      continue;
    }

    if (!currentEndpoint) continue;

    const summaryMatch = line.match(/^      summary:\s*(.+)\s*$/);
    if (summaryMatch) {
      currentEndpoint.summary = stripQuotes(summaryMatch[1]);
      continue;
    }

    const tagsMatch = line.match(/^      tags:\s*\[(.*)\]\s*$/);
    if (tagsMatch) {
      currentEndpoint.tags = tagsMatch[1]
        .split(',')
        .map((item) => stripQuotes(item))
        .map((item) => item.trim())
        .filter((item) => item.length > 0);
    }
  }

  return { title, version, servers, endpoints };
}

async function pathExists(filePath) {
  try {
    await stat(filePath);
    return true;
  } catch {
    return false;
  }
}

async function loadCliCatalog() {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), 'logline-cli-catalog-'));
  const tempFile = path.join(tempDir, 'cli-catalog.json');

  try {
    const result = spawnSync(
      'cargo',
      ['run', '-q', '-p', 'logline-cli', '--bin', 'logline-cli', '--', 'catalog', 'export', '--output', tempFile],
      {
        cwd: cliWorkspace,
        encoding: 'utf8',
      },
    );

    if (result.status !== 0) {
      const combinedOutput = `${result.stderr || ''}\n${result.stdout || ''}`;
      if (combinedOutput.includes("unrecognized subcommand 'catalog'")) {
        return {
          binary: 'logline',
          catalog_version: 'logline-cli.catalog.v0',
          commands: [],
        };
      }
      throw new Error(`failed to export CLI catalog: ${result.stderr || result.stdout || 'unknown error'}`);
    }

    const raw = await readFile(tempFile, 'utf8');
    return JSON.parse(raw);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

function buildCliCapabilities(cliCatalog) {
  const commands = Array.isArray(cliCatalog?.commands) ? cliCatalog.commands : [];

  return commands
    .filter((entry) => typeof entry.path === 'string' && entry.path.startsWith('logline '))
    .filter((entry) => !Array.isArray(entry.subcommands) || entry.subcommands.length === 0)
    .map((entry) => {
      const commandPath = entry.path.trim();
      const idSuffix = commandPath.replace(/\s+/g, '.').toLowerCase();
      const args = Array.isArray(entry.args) ? entry.args : [];

      return {
        capability_id: `cli.${idSuffix}`,
        surface: 'cli',
        service_id: 'logic-cli',
        command: commandPath,
        summary: entry.about ?? null,
        args: args.map((arg) => ({
          id: arg.id ?? null,
          long: arg.long ?? null,
          short: arg.short ?? null,
          required: Boolean(arg.required),
          help: arg.help ?? null,
        })),
      };
    });
}

async function buildApiCapabilities() {
  const capabilities = [];
  const sources = [];

  for (const spec of apiSpecs) {
    const absolutePath = path.join(rootDir, spec.file);
    if (!(await pathExists(absolutePath))) {
      continue;
    }

    const raw = await readFile(absolutePath, 'utf8');
    const parsed = parseOpenApiYaml(raw);

    sources.push({
      service_id: spec.service_id,
      source_file: spec.file,
      title: parsed.title,
      version: parsed.version,
      servers: parsed.servers,
      visibility: spec.surface,
      endpoint_count: parsed.endpoints.length,
    });

    for (const endpoint of parsed.endpoints) {
      capabilities.push({
        capability_id: `api.${spec.service_id}.${endpoint.method.toLowerCase()}.${slugPath(endpoint.path)}`,
        surface: 'api',
        service_id: spec.service_id,
        visibility: spec.surface,
        method: endpoint.method,
        path: endpoint.path,
        summary: endpoint.summary,
        tags: endpoint.tags,
        source_file: spec.file,
      });
    }
  }

  return { capabilities, sources };
}

function summarizeByService(capabilities) {
  const summary = {};
  for (const item of capabilities) {
    const service = item.service_id ?? 'unknown';
    summary[service] = (summary[service] ?? 0) + 1;
  }
  return summary;
}

function stableFingerprint(payload) {
  return JSON.stringify({
    catalog_version: payload.catalog_version,
    generated_by: payload.generated_by,
    summary: payload.summary,
    sources: payload.sources,
    capabilities: payload.capabilities,
  });
}

const cliCatalog = await loadCliCatalog();
const cliCapabilities = buildCliCapabilities(cliCatalog);
const { capabilities: apiCapabilities, sources: apiSources } = await buildApiCapabilities();

const allCapabilities = [...cliCapabilities, ...apiCapabilities]
  .sort((a, b) => String(a.capability_id).localeCompare(String(b.capability_id)));

let generatedAt = new Date().toISOString();
let existingOutput = null;

if (await pathExists(outputPath)) {
  try {
    existingOutput = JSON.parse(await readFile(outputPath, 'utf8'));
  } catch {
    existingOutput = null;
  }
}

const draftOutput = {
  catalog_version: 'logline.capability-catalog.v1',
  generated_at: generatedAt,
  generated_by: 'scripts/generate-capability-catalog.mjs',
  summary: {
    total_capabilities: allCapabilities.length,
    cli_capabilities: cliCapabilities.length,
    api_capabilities: apiCapabilities.length,
    by_service: summarizeByService(allCapabilities),
  },
  sources: {
    cli: {
      binary: cliCatalog?.binary ?? 'logline',
      catalog_version: cliCatalog?.catalog_version ?? 'logline-cli.catalog.v1',
      command_entries: Array.isArray(cliCatalog?.commands) ? cliCatalog.commands.length : 0,
    },
    apis: apiSources,
  },
  capabilities: allCapabilities,
};

if (existingOutput && stableFingerprint(existingOutput) === stableFingerprint(draftOutput)) {
  generatedAt = typeof existingOutput.generated_at === 'string'
    ? existingOutput.generated_at
    : generatedAt;
}

const output = {
  ...draftOutput,
  generated_at: generatedAt,
};

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(output, null, 2)}\n`);

console.log(`Capability catalog generated: ${path.relative(rootDir, outputPath)}`);
console.log(`Capabilities: ${allCapabilities.length} (cli=${cliCapabilities.length}, api=${apiCapabilities.length})`);
