#!/usr/bin/env node

import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, "..");

const args = process.argv.slice(2);
const applyHome = args.includes("--apply-home");
const topologyPathArg = args.find((arg) => !arg.startsWith("-"));
const topologyPath = topologyPathArg
  ? path.resolve(process.cwd(), topologyPathArg)
  : path.join(rootDir, "service_topology.json");

const topology = JSON.parse(await fs.readFile(topologyPath, "utf8"));

function fail(message) {
  console.error(`topology-gen: ${message}`);
  process.exit(1);
}

function required(value, key) {
  if (value === undefined || value === null || value === "") {
    fail(`missing required key: ${key}`);
  }
  return value;
}

function expandTilde(p) {
  return p.startsWith("~/") ? path.join(os.homedir(), p.slice(2)) : p;
}

function escapeSingleQuotes(value) {
  return value.replace(/'/g, `'\"'\"'`);
}

function indent(text, spaces) {
  const pad = " ".repeat(spaces);
  return text
    .split("\n")
    .map((line) => (line.length > 0 ? `${pad}${line}` : line))
    .join("\n");
}

function jsStringLiteral(value) {
  return JSON.stringify(value);
}

function jsValue(value, level = 0) {
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    const items = value.map((item) => `${" ".repeat(level + 2)}${jsValue(item, level + 2)}`);
    return `[\n${items.join(",\n")}\n${" ".repeat(level)}]`;
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) return "{}";
    const lines = entries.map(([k, v]) => `${" ".repeat(level + 2)}${k}: ${jsValue(v, level + 2)}`);
    return `{\n${lines.join(",\n")}\n${" ".repeat(level)}}`;
  }
  return JSON.stringify(value);
}

function renderPm2App(app) {
  const lines = [];
  lines.push("    {");
  lines.push(`      name: ${jsStringLiteral(app.name)},`);
  lines.push(`      script: ${jsStringLiteral(app.script)},`);

  if (app.mode === "doppler") {
    const command = required(app.command, `pm2_apps.${app.name}.command`);
    const shellArg = required(app.shell_args, `pm2_apps.${app.name}.shell_args`);
    lines.push(`      args: [${jsStringLiteral(shellArg)}, dopplerCommand(${jsStringLiteral(command)})],`);
  } else if (Array.isArray(app.args)) {
    lines.push(`      args: ${jsValue(app.args, 6)},`);
  } else if (app.args !== undefined) {
    lines.push(`      args: ${jsStringLiteral(app.args)},`);
  }

  if (app.cwd) lines.push(`      cwd: \`\${BASE}/${app.cwd}\`,`);
  if (app.interpreter) lines.push(`      interpreter: ${jsStringLiteral(app.interpreter)},`);
  if (app.autorestart !== undefined) lines.push(`      autorestart: ${app.autorestart},`);
  if (app.max_restarts !== undefined) lines.push(`      max_restarts: ${app.max_restarts},`);
  if (app.min_uptime !== undefined) lines.push(`      min_uptime: ${jsStringLiteral(app.min_uptime)},`);
  if (app.restart_delay !== undefined) lines.push(`      restart_delay: ${app.restart_delay},`);
  if (app.env) {
    lines.push("      env: {");
    for (const [key, value] of Object.entries(app.env)) {
      lines.push(`        ${key}: ${jsStringLiteral(value)},`);
    }
    lines.push("      },");
  }
  lines.push("    },");

  if (app.health_check) lines.push(`    // Health check: ${app.health_check}`);
  for (const note of app.notes ?? []) lines.push(`    // ${note}`);
  return lines.join("\n");
}

function renderEcosystemConfig(topo) {
  const meta = topo.meta;
  const doppler = meta.doppler;
  const apps = required(topo.pm2_apps, "pm2_apps");
  const appBlocks = apps.map((app) => renderPm2App(app)).join("\n\n");

  return `/**
 * AUTO-GENERATED FILE. DO NOT EDIT MANUALLY.
 * Source of truth: service_topology.json
 * Regenerate: node scripts/generate-topology-configs.mjs --apply-home
 */

const BASE = ${jsStringLiteral(required(meta.base_dir, "meta.base_dir"))};
const DOPPLER_BIN = process.env.DOPPLER_BIN || ${jsStringLiteral(required(doppler.bin, "meta.doppler.bin"))};
const DOPPLER_PROJECT = process.env.DOPPLER_PROJECT || ${jsStringLiteral(required(doppler.project, "meta.doppler.project"))};
const DOPPLER_CONFIG = process.env.DOPPLER_CONFIG || ${jsStringLiteral(required(doppler.config, "meta.doppler.config"))};

function dopplerCommand(command) {
  const escaped = command.replace(/'/g, \`'"'"'\`);
  return \`\${DOPPLER_BIN} run --project \${DOPPLER_PROJECT} --config \${DOPPLER_CONFIG} --command '\${escaped}'\`;
}

module.exports = {
  apps: [
${appBlocks}
  ],
};
`;
}

function yamlBoolean(value) {
  return value ? "true" : "false";
}

function renderCloudflaredConfig(topo, variant = "repo") {
  const cf = required(topo.meta.cloudflared, "meta.cloudflared");
  const ingress = required(topo.ingress, "ingress");
  const origin = variant === "active" ? (cf.active_origin_request ?? cf.origin_request) : cf.origin_request;

  const lines = [];
  if (variant === "repo") {
    lines.push("# Cloudflare Tunnel Configuration for logline.world");
    lines.push("#");
    lines.push("# AUTO-GENERATED FILE. DO NOT EDIT MANUALLY.");
    lines.push("# Source of truth: service_topology.json");
    lines.push("# Regenerate: node scripts/generate-topology-configs.mjs --apply-home");
    lines.push("");
  }
  lines.push(`tunnel: ${required(cf.tunnel, "meta.cloudflared.tunnel")}`);
  lines.push(`credentials-file: ${required(cf.credentials_file, "meta.cloudflared.credentials_file")}`);
  lines.push("");
  lines.push("ingress:");
  for (const entry of ingress) {
    if (entry.comment && variant === "repo") lines.push(`  # ${entry.comment}`);
    if (entry.hostname) lines.push(`  - hostname: ${entry.hostname}`);
    else lines.push("  -");
    lines.push(`    service: ${entry.service}`);
  }
  if (origin) {
    lines.push("");
    lines.push("originRequest:");
    if (origin.no_tls_verify !== undefined) {
      lines.push(`  noTLSVerify: ${yamlBoolean(origin.no_tls_verify)}`);
    }
    if (origin.connect_timeout) {
      lines.push(`  connectTimeout: ${origin.connect_timeout}`);
    }
    if (origin.http_request_timeout) {
      lines.push(`  httpRequestTimeout: ${origin.http_request_timeout}`);
    }
  }

  lines.push("");
  return lines.join("\n");
}

const generatedEcosystem = renderEcosystemConfig(topology);
const generatedRepoCloudflared = renderCloudflaredConfig(topology, "repo");
const generatedActiveCloudflared = renderCloudflaredConfig(topology, "active");

const ecoPath = path.resolve(rootDir, required(topology.meta.paths.ecosystem_config, "meta.paths.ecosystem_config"));
const repoCfPath = path.resolve(rootDir, required(topology.meta.paths.repo_cloudflared_config, "meta.paths.repo_cloudflared_config"));
const activeCfPath = path.resolve(expandTilde(required(topology.meta.paths.active_cloudflared_config, "meta.paths.active_cloudflared_config")));

await fs.writeFile(ecoPath, generatedEcosystem);
await fs.writeFile(repoCfPath, generatedRepoCloudflared);

if (applyHome) {
  await fs.writeFile(activeCfPath, generatedActiveCloudflared);
}

console.log(`Generated ${path.relative(rootDir, ecoPath)}`);
console.log(`Generated ${path.relative(rootDir, repoCfPath)}`);
if (applyHome) {
  console.log(`Generated ${activeCfPath}`);
} else {
  console.log("Skipped active cloudflared config (use --apply-home to write it).");
}
