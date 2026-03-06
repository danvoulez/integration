/**
 * AUTO-GENERATED FILE. DO NOT EDIT MANUALLY.
 * Source of truth: service_topology.json
 * Regenerate: node scripts/generate-topology-configs.mjs --apply-home
 */

const BASE = "/Users/ubl-ops/Integration";
const DOPPLER_BIN = process.env.DOPPLER_BIN || "doppler";
const DOPPLER_PROJECT = process.env.DOPPLER_PROJECT || "logline-ecosystem";
const DOPPLER_CONFIG = process.env.DOPPLER_CONFIG || "dev";

function dopplerCommand(command) {
  const escaped = command.replace(/'/g, `'"'"'`);
  return `${DOPPLER_BIN} run --project ${DOPPLER_PROJECT} --config ${DOPPLER_CONFIG} --command '${escaped}'`;
}

module.exports = {
  apps: [
    {
      name: "ollama",
      script: "/opt/homebrew/bin/ollama",
      args: "serve",
      interpreter: "none",
      autorestart: true,
      max_restarts: 10,
      min_uptime: "10s",
      restart_delay: 3000,
      env: {
        OLLAMA_HOST: "0.0.0.0:11434",
        OLLAMA_ORIGINS: "*",
      },
    },
    // Health check: curl http://localhost:11434/api/tags

    {
      name: "llm-gateway",
      script: "/bin/bash",
      args: ["-lc", dopplerCommand("export LLM_API_KEY=\"${LLM_API_KEY:-${LLM_GATEWAY_API_KEY:-}}\" && cargo run --release --bin llm-gateway")],
      cwd: `${BASE}/llm-gateway.logline.world`,
      interpreter: "none",
      autorestart: true,
      max_restarts: 10,
      min_uptime: "10s",
      restart_delay: 5000,
      env: {
        RUST_LOG: "info",
      },
    },
    // Health check: curl http://localhost:7700/health
    // Modes: genius (premium), fast (cheap), code (local+fallback)

    {
      name: "code247",
      script: "/bin/bash",
      args: ["-lc", dopplerCommand("export LLM_GATEWAY_API_KEY=\"${LLM_GATEWAY_API_KEY:-${LLM_API_KEY:-}}\" && cargo run --release --bin dual-agents-rust")],
      cwd: `${BASE}/code247.logline.world`,
      interpreter: "none",
      autorestart: true,
      max_restarts: 10,
      min_uptime: "10s",
      restart_delay: 5000,
      env: {
        RUST_LOG: "info",
        HEALTH_PORT: "4001",
      },
    },
    // Health check: curl http://localhost:4001/health

    {
      name: "edge-control",
      script: "/bin/bash",
      args: ["-lc", dopplerCommand("cargo run --release")],
      cwd: `${BASE}/edge-control.logline.world`,
      interpreter: "none",
      autorestart: true,
      max_restarts: 10,
      min_uptime: "10s",
      restart_delay: 5000,
      env: {
        RUST_LOG: "info",
        EDGE_CONTROL_PORT: "18080",
      },
    },
    // Health check: curl http://localhost:18080/health

    {
      name: "obs-api",
      script: "/bin/bash",
      args: ["-lc", dopplerCommand("npm run build && npm run start -- --port 3001")],
      cwd: `${BASE}/obs-api.logline.world`,
      interpreter: "none",
      autorestart: true,
      max_restarts: 10,
      min_uptime: "10s",
      restart_delay: 5000,
      env: {
        NODE_ENV: "production",
        PORT: "3001",
      },
    },
    // Health check: curl http://localhost:3001/api/health
  ],
};
