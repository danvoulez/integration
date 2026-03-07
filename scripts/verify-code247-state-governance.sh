#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[code247-state-governance] root=${ROOT_DIR}"

cargo test \
  --manifest-path "${ROOT_DIR}/code247.logline.world/Cargo.toml" \
  sync_http_

echo "[code247-state-governance] ok"
