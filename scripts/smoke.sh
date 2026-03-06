#!/usr/bin/env bash
set -euo pipefail

TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-10}"
CHECK_EXTERNAL="${CHECK_EXTERNAL:-1}"
CHECK_SUPABASE="${CHECK_SUPABASE:-1}"

FAILURES=0

check_200() {
  local name="$1"
  local url="$2"
  local code
  code="$(curl -sS -o /dev/null -w "%{http_code}" --max-time "${TIMEOUT_SECONDS}" "${url}" || true)"
  if [[ "${code}" == "200" ]]; then
    echo "[PASS] ${name}: ${url}"
  else
    echo "[FAIL] ${name}: ${url} (status=${code})"
    FAILURES=$((FAILURES + 1))
  fi
}

check_reachable() {
  local name="$1"
  local url="$2"
  local code
  code="$(curl -sS -o /dev/null -w "%{http_code}" --max-time "${TIMEOUT_SECONDS}" "${url}" || true)"
  if [[ "${code}" =~ ^[234][0-9][0-9]$ ]]; then
    echo "[PASS] ${name}: ${url} (status=${code})"
  else
    echo "[FAIL] ${name}: ${url} (status=${code})"
    FAILURES=$((FAILURES + 1))
  fi
}

echo "== Local health checks =="
check_200 "llm-gateway" "http://127.0.0.1:7700/health"
check_200 "code247" "http://127.0.0.1:4001/health"
check_200 "edge-control" "http://127.0.0.1:18080/health"
check_200 "obs-api" "http://127.0.0.1:3001/api/health"

if [[ "${CHECK_EXTERNAL}" == "1" ]]; then
  echo "== External health checks =="
  check_200 "llm-gateway external" "https://llm-gateway.logline.world/health"
  check_200 "code247 external" "https://code247.logline.world/health"
  check_200 "edge-control external" "https://edge-control.logline.world/health"
  check_200 "obs-api external" "https://obs-api.logline.world/api/health"
fi

if [[ "${CHECK_SUPABASE}" == "1" ]]; then
  echo "== Supabase reachability =="
  if [[ -z "${SUPABASE_URL:-}" ]]; then
    echo "[SKIP] SUPABASE_URL is not set in environment."
  else
    check_reachable "supabase auth health" "${SUPABASE_URL%/}/auth/v1/health"
  fi
fi

if [[ "${FAILURES}" -gt 0 ]]; then
  echo "Smoke failed with ${FAILURES} failing checks."
  exit 1
fi

echo "Smoke passed."
