#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');
const catalogPath = path.join(rootDir, 'contracts', 'generated', 'capability-catalog.v1.json');
const outputPath = process.argv[2]
  ? path.resolve(process.cwd(), process.argv[2])
  : path.join(rootDir, 'contracts', 'generated', 'cookbook.capabilities.v1.md');

function runCatalogGenerator() {
  const result = spawnSync('node', [path.join(rootDir, 'scripts', 'generate-capability-catalog.mjs')], {
    cwd: rootDir,
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || 'failed to generate capability catalog');
  }
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function groupBy(items, keyFn) {
  const map = new Map();
  for (const item of items) {
    const key = keyFn(item);
    if (!map.has(key)) {
      map.set(key, []);
    }
    map.get(key).push(item);
  }
  return map;
}

function buildMarkdown(catalog) {
  const capabilities = asArray(catalog.capabilities);
  const cliCaps = capabilities.filter((item) => item.surface === 'cli');
  const apiCaps = capabilities.filter((item) => item.surface === 'api');
  const byService = groupBy(capabilities, (item) => item.service_id ?? 'unknown');

  const lines = [
    '# Cookbook Canônico (Gerado)',
    '',
    `Gerado em: \`${new Date().toISOString()}\``,
    `Fonte: \`contracts/generated/capability-catalog.v1.json\``,
    '',
    '## Resumo',
    `- Total de capacidades: **${capabilities.length}**`,
    `- CLI: **${cliCaps.length}**`,
    `- API: **${apiCaps.length}**`,
    '',
    '## Capacidades CLI',
    '',
    '| Comando | Resumo |',
    '|---|---|',
  ];

  for (const item of cliCaps) {
    const command = item.command ?? item.capability_id ?? 'unknown';
    const summary = item.summary ?? '-';
    lines.push(`| \`${command}\` | ${summary} |`);
  }

  lines.push('', '## Capacidades API por Serviço', '');

  const sortedServices = [...byService.keys()].sort((a, b) => a.localeCompare(b));
  for (const service of sortedServices) {
    const serviceItems = byService
      .get(service)
      .filter((item) => item.surface === 'api')
      .sort((a, b) => String(a.capability_id).localeCompare(String(b.capability_id)));
    if (serviceItems.length === 0) {
      continue;
    }

    lines.push(`### ${service}`, '');
    lines.push('| Método | Path | Resumo |');
    lines.push('|---|---|---|');
    for (const item of serviceItems) {
      lines.push(`| \`${item.method ?? '-'}\` | \`${item.path ?? '-'}\` | ${item.summary ?? '-'} |`);
    }
    lines.push('');
  }

  lines.push('## Uso Operacional');
  lines.push('- Atualizar catálogo: `node scripts/generate-capability-catalog.mjs`');
  lines.push('- Atualizar cookbook: `node scripts/generate-cookbook.mjs`');
  lines.push('- Pipeline severo: `./scripts/integration-severe.sh`');
  lines.push('');
  lines.push('> Este arquivo é gerado automaticamente. Não editar manualmente.');
  lines.push('');

  return `${lines.join('\n')}\n`;
}

runCatalogGenerator();
const catalogRaw = await readFile(catalogPath, 'utf8');
const catalog = JSON.parse(catalogRaw);
const markdown = buildMarkdown(catalog);

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, markdown);

console.log(`Cookbook generated: ${path.relative(rootDir, outputPath)}`);
