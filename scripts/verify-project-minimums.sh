#!/usr/bin/env bash
# Verify each key project has the canonical minimum documents and manifest.

set -euo pipefail

failures=()

check_project() {
  local project="$1"
  shift
  local missing=()

  for file in "$@"; do
    local candidate
    if [[ "$project" == "." ]]; then
      candidate="$file"
    else
      candidate="$project/$file"
    fi

    if [[ ! -f "$candidate" ]]; then
      missing+=("$candidate")
    fi
  done

  if [[ "${#missing[@]}" -gt 0 ]]; then
    failures+=("$project: missing ${missing[*]}")
  fi
}

check_project "." \
  ".code247/workspace.manifest.json" \
  "LOGLINE_ECOSYSTEM_NORMATIVE_BASE.md" \
  "INTEGRATION_BLUEPRINT.md" \
  "SERVICE_TOPOLOGY.md" \
  "INFRA_RUNBOOK.md" \
  "SUPABASE_FOUNDATION.md"

check_project "code247.logline.world" \
  ".code247/workspace.manifest.json" \
  "README.md" \
  "Consumo-de-Projetos.md" \
  "CODE247 × VVTV — Integration Contract v1.md" \
  "systemd/dual-agents.service"

check_project "llm-gateway.logline.world" \
  ".code247/workspace.manifest.json" \
  "README.md" \
  "BLUEPRINT.md" \
  "RUNBOOK.md" \
  "openapi.yaml"

check_project "logic.logline.world" \
  ".code247/workspace.manifest.json" \
  "README.md" \
  "docs/ARCHITECTURE.md" \
  "docs/OPERATIONS.md" \
  "docs/DEPLOYMENT.md" \
  "docs/SECURITY.md"

check_project "obs-api.logline.world" \
  ".code247/workspace.manifest.json" \
  "README.md" \
  "docs/README.md" \
  "docs/TEMPLATE_CONTRACT.md" \
  "docs/TESTING.md" \
  "docs/TROUBLESHOOTING.md" \
  "docs/SETTINGS_CASCADE.md"

if [[ "${#failures[@]}" -gt 0 ]]; then
  echo "❌ Missing canonical artifacts:"
  for entry in "${failures[@]}"; do
    echo "- $entry"
  done
  exit 1
fi

echo "✅ All key projects meet the canonical minimum."
