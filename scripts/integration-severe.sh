#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_PATH="${REPORT_PATH:-${ROOT_DIR}/artifacts/integration-severe-report.json}"

echo "[integration-severe] root=${ROOT_DIR}"
echo "[integration-severe] report=${REPORT_PATH}"

cargo run \
  --manifest-path "${ROOT_DIR}/logic.logline.world/Cargo.toml" \
  -p logline-cli \
  --bin logline-cli \
  -- \
  harness run \
  --root "${ROOT_DIR}" \
  --report "${REPORT_PATH}"

echo "[integration-severe] ok"
