#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const schemaPath = process.argv[2]
  ? path.resolve(process.cwd(), process.argv[2])
  : path.join(rootDir, 'canon', 'workspace-ast', '0.1', 'workspace.manifest.schema.json');

const outputPath = process.argv[3]
  ? path.resolve(process.cwd(), process.argv[3])
  : path.join(rootDir, 'logic.logline.world', 'schemas', 'workspace.manifest.generated.ts');

function toTypeName(raw) {
  return String(raw)
    .replace(/[^a-zA-Z0-9]+/g, ' ')
    .trim()
    .split(/\s+/)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

function refName(ref) {
  const tail = String(ref).split('/').pop() ?? 'Unknown';
  return toTypeName(tail);
}

function schemaToType(schema, ctx) {
  if (!schema || typeof schema !== 'object') return 'unknown';
  if (schema.$ref) return refName(schema.$ref);
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum.map((item) => JSON.stringify(item)).join(' | ');
  }
  if (Array.isArray(schema.oneOf) && schema.oneOf.length > 0) {
    return schema.oneOf.map((item) => schemaToType(item, ctx)).join(' | ');
  }
  if (Array.isArray(schema.anyOf) && schema.anyOf.length > 0) {
    return schema.anyOf.map((item) => schemaToType(item, ctx)).join(' | ');
  }
  if (schema.type === 'array') {
    const itemType = schemaToType(schema.items ?? {}, ctx);
    return `(${itemType})[]`;
  }
  if (schema.type === 'object' || schema.properties) {
    const properties = schema.properties ?? {};
    const required = new Set(Array.isArray(schema.required) ? schema.required : []);
    const fields = Object.entries(properties).map(([name, propSchema]) => {
      const optional = required.has(name) ? '' : '?';
      return `${JSON.stringify(name)}${optional}: ${schemaToType(propSchema, ctx)};`;
    });
    if (schema.additionalProperties) {
      const extraType =
        schema.additionalProperties === true
          ? 'unknown'
          : schemaToType(schema.additionalProperties, ctx);
      fields.push(`[key: string]: ${extraType};`);
    }
    return `{\n${fields.map((line) => `  ${line}`).join('\n')}\n}`;
  }
  switch (schema.type) {
    case 'string':
      return 'string';
    case 'integer':
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'null':
      return 'null';
    default:
      return 'unknown';
  }
}

function buildTypeFile(schema) {
  const definitions = schema.definitions ?? schema.$defs ?? {};
  const lines = [
    '/* eslint-disable */',
    '// AUTO-GENERATED FILE. DO NOT EDIT MANUALLY.',
    `// Source: ${path.relative(rootDir, schemaPath)}`,
    '',
  ];

  const defEntries = Object.entries(definitions).sort(([a], [b]) => a.localeCompare(b));
  for (const [name, defSchema] of defEntries) {
    const typeName = toTypeName(name);
    lines.push(`export type ${typeName} = ${schemaToType(defSchema, { definitions })};`, '');
  }

  const rootName = toTypeName(schema.title || 'workspace_manifest');
  lines.push(`export type ${rootName} = ${schemaToType(schema, { definitions })};`);
  lines.push(`export type WorkspaceManifest = ${rootName};`, '');

  return `${lines.join('\n')}\n`;
}

const raw = await readFile(schemaPath, 'utf8');
const schema = JSON.parse(raw);
const output = buildTypeFile(schema);

await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, output);

console.log(`TypeScript schema bindings generated: ${path.relative(rootDir, outputPath)}`);
