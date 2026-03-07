#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_PATH="${REPORT_PATH:-${ROOT_DIR}/artifacts/operations-verify-report.json}"
SUMMARY_PATH="${REPORT_PATH%.json}.md"

echo "[verify-operations] root=${ROOT_DIR}"
echo "[verify-operations] report=${REPORT_PATH}"

cargo run \
  --manifest-path "${ROOT_DIR}/logic.logline.world/Cargo.toml" \
  -p logline-cli \
  --bin logline-cli \
  -- \
  harness run \
  --root "${ROOT_DIR}" \
  --report "${REPORT_PATH}"

echo "[verify-operations] ok"
echo "[verify-operations] summary=${SUMMARY_PATH}"
