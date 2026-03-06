#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOGIC_DIR="${ROOT_DIR}/logic.logline.world"
DOPPLER_PROJECT="${DOPPLER_PROJECT:-logline-ecosystem}"
DOPPLER_CONFIG="${DOPPLER_CONFIG:-dev}"
USE_DOPPLER="${USE_DOPPLER:-1}"

FROM="${FROM:-$(date -u -v-1d '+%Y-%m-%dT00:00:00Z' 2>/dev/null || date -u -d '1 day ago' '+%Y-%m-%dT00:00:00Z')}"
TO="${TO:-$(date -u '+%Y-%m-%dT00:00:00Z')}"
PRICE_CARD_VERSION="${PRICE_CARD_VERSION:-}"

CMD=(cargo run -q -p logline-cli --bin logline-cli -- fuel reconcile --from "${FROM}" --to "${TO}" --json)
if [[ -n "${PRICE_CARD_VERSION}" ]]; then
  CMD+=(--price-card-version "${PRICE_CARD_VERSION}")
fi

echo "[fuel-reconcile] window: ${FROM} -> ${TO}"

if [[ "${USE_DOPPLER}" == "1" ]] && command -v doppler >/dev/null 2>&1; then
  DOPPLER_CMD="cd '${LOGIC_DIR}' && ${CMD[*]}"
  doppler run --project "${DOPPLER_PROJECT}" --config "${DOPPLER_CONFIG}" --command "${DOPPLER_CMD}"
else
  (
    cd "${LOGIC_DIR}"
    "${CMD[@]}"
  )
fi
