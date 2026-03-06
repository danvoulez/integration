export type ProjectStatus = "active" | "paused" | "archived" | "draft";

export interface Project {
  id: string;
  name: string;
  description: string;
  status: ProjectStatus;
  tech: string[];
  updatedAt: string;
  url?: string;
  repo?: string;
  notes?: string;
}

export interface App {
  id: string;
  name: string;
  subtitle: string;
  icon: string;
  color: string;
  projects: Project[];
}

export interface EcossistemaItem {
  id: string;
  label: string;
  value: string;
  icon: string;
  color: string;
  description: string;
}

export const ecossistema: EcossistemaItem[] = [
  {
    id: "logline",
    label: "LogLine Protocol",
    value: "v0.3.1",
    icon: "📋",
    color: "#1a73e8",
    description: "Verifiable action protocol — cryptographic audit trail for all system events",
  },
  {
    id: "jsonatomic",
    label: "JSON✯Atomic",
    value: "v0.2.0",
    icon: "⚛️",
    color: "#34a853",
    description: "Cryptographic atom for structured data with zero-knowledge proofs",
  },
  {
    id: "tdln",
    label: "TDLN",
    value: "v1.0.0-rc1",
    icon: "🧠",
    color: "#9c27b0",
    description: "Deterministic Translation Layer — natural language to typed semantic structures",
  },
  {
    id: "sirp",
    label: "SIRP",
    value: "v0.1.4",
    icon: "🌐",
    color: "#f57c00",
    description: "Semantic Intent Routing Protocol — intelligent request distribution",
  },
  {
    id: "chip",
    label: "Chip as Code",
    value: "v0.4.2",
    icon: "💾",
    color: "#e53935",
    description: "Hardware abstraction as composable protocol units",
  },
  {
    id: "ubl",
    label: "UBL Core",
    value: "v0.1.0",
    icon: "🏗️",
    color: "#00897b",
    description: "Universal Business Ledger — physics-first economic infrastructure",
  },
  {
    id: "aurea",
    label: "AUREA",
    value: "v0.2.1",
    icon: "✨",
    color: "#f9a825",
    description: "AI governance and accountability layer for enterprise deployments",
  },
];

export const apps: App[] = [
  {
    id: "voulezvous",
    name: "Voulezvous",
    subtitle: "dvoulez@gmail.com",
    icon: "📺",
    color: "#1a73e8",
    projects: [
      {
        id: "vvtv",
        name: "VVTV Platform",
        description: "Live streaming and VOD platform built on Rust + Cloudflare Workers",
        status: "active",
        tech: ["Rust", "Cloudflare", "Next.js"],
        updatedAt: "2026-03-04",
        url: "https://voulezvous.tv",
        repo: "vvtv-platform-local",
        notes: "Main production platform. Deploy via Cloudflare Pages.",
      },
      {
        id: "messenger",
        name: "VV Messenger",
        description: "Real-time encrypted messaging for Voulezvous ecosystem",
        status: "active",
        tech: ["Next.js", "Supabase", "WebSockets"],
        updatedAt: "2026-03-01",
        repo: "messenger",
      },
      {
        id: "faturas",
        name: "Faturas VV",
        description: "Invoicing and billing system for VV Clube members",
        status: "paused",
        tech: ["Next.js", "Stripe"],
        updatedAt: "2026-02-10",
        notes: "On hold pending payment gateway decision.",
      },
      {
        id: "registry-ui",
        name: "Registry UI",
        description: "Admin dashboard for Chip Registry entries",
        status: "draft",
        tech: ["Next.js", "TailwindCSS"],
        updatedAt: "2026-02-22",
        repo: "ubl-registry-ui-initial-20260222-040828",
      },
    ],
  },
  {
    id: "logline-core",
    name: "LogLine Core",
    subtitle: "dan@danvoulez.com",
    icon: "📋",
    color: "#34a853",
    projects: [
      {
        id: "logline-cli",
        name: "LogLine CLI / API / MCP",
        description: "Command-line interface, REST API, and Model Context Protocol server for LogLine",
        status: "active",
        tech: ["Rust", "Axum", "MCP"],
        updatedAt: "2026-03-03",
        repo: "LogLine-CLI-API-MCP",
      },
      {
        id: "logline-crates",
        name: "Crates LogLine",
        description: "Rust crate ecosystem for LogLine — json-atomic, ubl-auth, lllv-index",
        status: "active",
        tech: ["Rust", "crates.io"],
        updatedAt: "2026-03-02",
        repo: "crates",
      },
      {
        id: "logline-extension",
        name: "Browser Extension",
        description: "Chrome extension that injects LogLine protocol into any web workflow",
        status: "draft",
        tech: ["TypeScript", "Chrome APIs"],
        updatedAt: "2026-01-30",
        repo: "extension logline",
      },
    ],
  },
  {
    id: "github-danvoulez",
    name: "GitHub",
    subtitle: "danvoulez",
    icon: "🐙",
    color: "#333",
    projects: [
      {
        id: "sirp-unified",
        name: "SIRP Unified",
        description: "Semantic Intent Routing Protocol — unified repo after consolidation",
        status: "active",
        tech: ["Rust", "Cloudflare"],
        updatedAt: "2026-01-04",
        repo: "sirp-unified",
      },
      {
        id: "tdln-core",
        name: "TDLN Core",
        description: "Deterministic Translation Layer — core compiler and runtime",
        status: "active",
        tech: ["Rust"],
        updatedAt: "2026-01-09",
        repo: "tdln-core",
      },
      {
        id: "aurea",
        name: "AUREA",
        description: "AI governance layer — policy gates, accountability chains",
        status: "paused",
        tech: ["Rust", "Python"],
        updatedAt: "2026-02-17",
        repo: "AUREA",
      },
      {
        id: "chip-registry",
        name: "Chip Registry",
        description: "Global registry for Chip as Code units — backend + frontend",
        status: "active",
        tech: ["Rust", "Next.js", "Cloudflare D1"],
        updatedAt: "2026-02-22",
        repo: "ChipRegistryBackendFrontend",
      },
    ],
  },
  {
    id: "ngrok",
    name: "ngrok",
    subtitle: "dan@danvoulez.com",
    icon: "🔗",
    color: "#f57c00",
    projects: [
      {
        id: "tunnel-dev",
        name: "Dev Tunnel",
        description: "Local development tunnel for testing webhooks and mobile APIs",
        status: "active",
        tech: ["ngrok", "Cloudflare Tunnel"],
        updatedAt: "2026-03-05",
        notes: "Runs on port 3000. Auto-start via launchd.",
      },
      {
        id: "tunnel-staging",
        name: "Staging Tunnel",
        description: "Persistent staging endpoint for external integrations",
        status: "paused",
        tech: ["ngrok"],
        updatedAt: "2026-02-01",
      },
    ],
  },
  {
    id: "openai",
    name: "OpenAI",
    subtitle: "dvoulez@gmail.com",
    icon: "🤖",
    color: "#10a37f",
    projects: [
      {
        id: "llm-gateway",
        name: "LLM Gateway",
        description: "Unified gateway routing requests across OpenAI, Anthropic, and local models",
        status: "active",
        tech: ["Python", "FastAPI", "Redis"],
        updatedAt: "2026-03-01",
        repo: "llm-gateway",
      },
      {
        id: "office-llm",
        name: "Office LLM",
        description: "One-pack LLM integration for back-office automation tasks",
        status: "active",
        tech: ["Python", "OpenAI API"],
        updatedAt: "2026-01-05",
        repo: "office-llm-onepack",
      },
    ],
  },
  {
    id: "hetzner",
    name: "Hetzner",
    subtitle: "Infrastructure",
    icon: "🖥️",
    color: "#d50000",
    projects: [
      {
        id: "srv-main",
        name: "srv / Main Server",
        description: "Primary VPS — runs Postgres, Redis, PM2 processes, and reverse proxy",
        status: "active",
        tech: ["Ubuntu", "PM2", "Nginx", "Docker"],
        updatedAt: "2026-03-05",
        notes: "CX31 instance in Nuremberg. SSH via ~/.ssh/hetzner_main.",
      },
      {
        id: "obs-api",
        name: "OBS API Server",
        description: "Dedicated server for OBS WebSocket API and stream control",
        status: "paused",
        tech: ["Node.js", "OBS WebSocket"],
        updatedAt: "2026-01-20",
      },
    ],
  },
  {
    id: "pypi",
    name: "PyPI",
    subtitle: "danvoulez",
    icon: "🐍",
    color: "#3776ab",
    projects: [
      {
        id: "logline-py",
        name: "logline-python",
        description: "Python SDK for LogLine protocol — for data pipelines and ML workflows",
        status: "draft",
        tech: ["Python", "PyPI"],
        updatedAt: "2026-01-15",
      },
      {
        id: "sirp-py",
        name: "sirp-client",
        description: "Python client for SIRP semantic routing",
        status: "draft",
        tech: ["Python", "aiohttp"],
        updatedAt: "2026-01-10",
      },
    ],
  },
];

// ── Fuel Consumption Metrics ──

export interface FuelMetric {
  id: string;
  label: string;
  value: number;      // current period consumption
  max: number;        // capacity / budget
  unit: string;
  trend: number;      // % change from previous period (positive = increase)
}

export const fuelByApps: FuelMetric[] = [
  { id: "voulezvous", label: "Voulezvous", value: 12400, max: 20000, unit: "req/h", trend: 8.2 },
  { id: "logline-core", label: "LogLine Core", value: 8700, max: 15000, unit: "req/h", trend: -3.1 },
  { id: "github", label: "GitHub", value: 2300, max: 5000, unit: "req/h", trend: 12.5 },
  { id: "ngrok", label: "ngrok", value: 940, max: 3000, unit: "req/h", trend: 1.4 },
  { id: "openai", label: "OpenAI", value: 6100, max: 10000, unit: "req/h", trend: 22.7 },
  { id: "hetzner", label: "Hetzner", value: 4500, max: 8000, unit: "req/h", trend: -0.8 },
  { id: "pypi", label: "PyPI", value: 180, max: 2000, unit: "req/h", trend: 0.0 },
];

export const fuelByTenants: FuelMetric[] = [
  { id: "vv-prod", label: "VV Production", value: 18200, max: 30000, unit: "req/h", trend: 5.6 },
  { id: "vv-staging", label: "VV Staging", value: 3400, max: 10000, unit: "req/h", trend: -12.0 },
  { id: "ubl-dev", label: "UBL Dev", value: 7800, max: 15000, unit: "req/h", trend: 18.3 },
  { id: "external", label: "External APIs", value: 5700, max: 8000, unit: "req/h", trend: 3.2 },
];

export const fuelByUsers: FuelMetric[] = [
  { id: "dan", label: "Dan Voulez", value: 14200, max: 25000, unit: "req/h", trend: 7.1 },
  { id: "system", label: "System / Cron", value: 9400, max: 20000, unit: "req/h", trend: -1.3 },
  { id: "api-keys", label: "API Keys", value: 8300, max: 12000, unit: "req/h", trend: 15.8 },
  { id: "anonymous", label: "Anonymous", value: 3200, max: 5000, unit: "req/h", trend: 2.4 },
];

export const totalFuel = {
  consumed: 35120,
  budget: 62000,
  unit: "req/h",
};
