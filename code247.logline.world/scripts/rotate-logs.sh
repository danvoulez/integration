#!/usr/bin/env bash
set -euo pipefail
LOG="data/audit.log"
if [[ -f "$LOG" ]]; then
  mv "$LOG" "${LOG}.$(date +%Y%m%d%H%M%S)"
fi
