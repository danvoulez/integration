#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CANON_DIR="${ROOT_DIR}/canon/workspace-ast/0.1"
LOGIC_SCHEMA_DIR="${ROOT_DIR}/logic.logline.world/schemas"

CHECK_ONLY=0
if [[ "${1:-}" == "--check" ]]; then
  CHECK_ONLY=1
fi

declare -a FILES=(
  "workspace.manifest.schema.json"
  "workspace.manifest.strict.schema.json"
  "inputs.linear.schema.json"
)

drift=0
for file in "${FILES[@]}"; do
  src="${CANON_DIR}/${file}"
  dst="${LOGIC_SCHEMA_DIR}/${file}"

  if [[ ! -f "${src}" ]]; then
    echo "[canon-sync] missing source: ${src}" >&2
    exit 1
  fi

  if [[ ! -f "${dst}" ]]; then
    echo "[canon-sync] missing destination: ${dst}" >&2
    drift=1
    continue
  fi

  if ! cmp -s "${src}" "${dst}"; then
    echo "[canon-sync] drift detected: ${file}"
    drift=1
    if [[ "${CHECK_ONLY}" -eq 0 ]]; then
      cp "${src}" "${dst}"
      echo "[canon-sync] synced: ${file}"
    fi
  fi
done

if [[ "${CHECK_ONLY}" -eq 1 && "${drift}" -eq 1 ]]; then
  echo "[canon-sync] schema drift detected. run: ./scripts/sync-canon-schemas.sh" >&2
  exit 1
fi

if [[ "${CHECK_ONLY}" -eq 0 ]]; then
  node "${ROOT_DIR}/scripts/generate-workspace-manifest-types.mjs"
fi

echo "[canon-sync] ok"
