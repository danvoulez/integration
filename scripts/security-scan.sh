#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="$ROOT_DIR/artifacts"
REPORT_FILE="$ARTIFACT_DIR/security-scan-report.txt"
AUDIT_CONFIG="$ROOT_DIR/audit.toml"

mkdir -p "$ARTIFACT_DIR"

failures=()
audit_ignore_args=()

record() {
  printf '%s\n' "$1" | tee -a "$REPORT_FILE"
}

run_scan() {
  local label="$1"
  shift

  record ""
  record "== $label =="
  if "$@" >>"$REPORT_FILE" 2>&1; then
    record "status=ok"
  else
    local code=$?
    failures+=("$label (exit $code)")
    record "status=failed exit=$code"
  fi
}

: >"$REPORT_FILE"
record "security_scan_started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
record "root=$ROOT_DIR"

if [[ -f "$AUDIT_CONFIG" ]]; then
  while IFS= read -r advisory; do
    audit_ignore_args+=(--ignore "$advisory")
  done < <(grep -Eo 'RUSTSEC-[0-9]{4}-[0-9]{4}' "$AUDIT_CONFIG" | sort -u)
fi

while IFS= read -r lockfile; do
  rel="${lockfile#$ROOT_DIR/}"
  run_scan \
    "cargo_audit:$rel" \
    cargo audit "${audit_ignore_args[@]}" --file "$lockfile"
done < <(find "$ROOT_DIR" -name Cargo.lock | sort)

while IFS= read -r lockfile; do
  rel="${lockfile#$ROOT_DIR/}"
  pkg_dir="$(dirname "$lockfile")"
  run_scan \
    "npm_audit:$rel" \
    npm audit --package-lock-only --omit=dev --audit-level=high --prefix "$pkg_dir"
done < <(find "$ROOT_DIR" -name package-lock.json | sort)

record ""
if [[ "${#failures[@]}" -gt 0 ]]; then
  record "security_scan_status=failed"
  for failure in "${failures[@]}"; do
    record "failure=$failure"
  done
  exit 1
fi

record "security_scan_status=ok"
