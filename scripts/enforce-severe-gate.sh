#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

resolve_base_ref() {
  if [[ -n "${GITHUB_BASE_REF:-}" ]]; then
    if git rev-parse --verify "origin/${GITHUB_BASE_REF}" >/dev/null 2>&1; then
      printf '%s' "origin/${GITHUB_BASE_REF}"
      return
    fi
    if git rev-parse --verify "${GITHUB_BASE_REF}" >/dev/null 2>&1; then
      printf '%s' "${GITHUB_BASE_REF}"
      return
    fi
  fi

  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    printf '%s' "origin/main"
    return
  fi
  if git rev-parse --verify main >/dev/null 2>&1; then
    printf '%s' "main"
    return
  fi
  if git rev-parse --verify HEAD~1 >/dev/null 2>&1; then
    printf '%s' "HEAD~1"
    return
  fi

  printf ''
}

BASE_REF="$(resolve_base_ref)"
if [[ -z "${BASE_REF}" ]]; then
  echo "[severe-gate] skip: unable to resolve git base ref"
  exit 0
fi

CHANGED_FILES=()
while IFS= read -r file; do
  [[ -n "${file}" ]] && CHANGED_FILES+=("${file}")
done < <(git diff --name-only "${BASE_REF}...HEAD")
if [[ "${#CHANGED_FILES[@]}" -eq 0 ]]; then
  echo "[severe-gate] no file changes detected"
  exit 0
fi

SENSITIVE_REGEX='^(code247\.logline\.world/src/(main|api_rs|persistence_rs)\.rs|edge-control\.logline\.world/src/(auth|middleware|state_store|main|config)\.rs|logic\.logline\.world/supabase/migrations/.*fuel.*\.sql|obs-api\.logline\.world/lib/obs/fuel\.ts)$'
SEVERE_SUITE_REGEX='^(logic\.logline\.world/crates/logline-cli/src/commands/harness\.rs|scripts/(integration-severe|verify-operations|enforce-severe-gate|smoke-obs-api-auth|smoke-llm-gateway-auth)\.sh)$'

sensitive_hits=()
severe_hits=()
for file in "${CHANGED_FILES[@]}"; do
  if [[ "${file}" =~ ${SENSITIVE_REGEX} ]]; then
    sensitive_hits+=("${file}")
  fi
  if [[ "${file}" =~ ${SEVERE_SUITE_REGEX} ]]; then
    severe_hits+=("${file}")
  fi
done

if [[ "${#sensitive_hits[@]}" -eq 0 ]]; then
  echo "[severe-gate] no sensitive integration deltas detected"
  exit 0
fi

if [[ "${#severe_hits[@]}" -eq 0 ]]; then
  echo "[severe-gate] ERROR: sensitive integration files changed but severe suite artifacts were not updated" >&2
  echo "[severe-gate] Sensitive files:" >&2
  printf '  - %s\n' "${sensitive_hits[@]}" >&2
  echo "[severe-gate] Required updates include one of:" >&2
  echo "  - logic.logline.world/crates/logline-cli/src/commands/harness.rs" >&2
  echo "  - scripts/integration-severe.sh" >&2
  echo "  - scripts/verify-operations.sh" >&2
  echo "  - scripts/enforce-severe-gate.sh" >&2
  echo "  - scripts/smoke-obs-api-auth.sh" >&2
  echo "  - scripts/smoke-llm-gateway-auth.sh" >&2
  exit 1
fi

echo "[severe-gate] ok"
echo "[severe-gate] sensitive files touched: ${#sensitive_hits[@]}"
echo "[severe-gate] severe suite files touched: ${#severe_hits[@]}"
