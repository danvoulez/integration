#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[contracts] validating canonical files from ${ROOT_DIR}"

echo "[contracts] generating capability catalog"
node "${ROOT_DIR}/scripts/generate-capability-catalog.mjs"

cargo run \
  --manifest-path "${ROOT_DIR}/logic.logline.world/Cargo.toml" \
  -p logline-cli \
  --bin contracts_validate \
  -- \
  --root "${ROOT_DIR}"

echo "[contracts] validation complete"
