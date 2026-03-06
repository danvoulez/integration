#!/usr/bin/env bash
set -euo pipefail

# LogLine Ecosystem — Doppler bootstrap
#
# Usage:
#   ./scripts/doppler-setup.sh
#   ./scripts/doppler-setup.sh --project logline-ecosystem --config dev
#   ./scripts/doppler-setup.sh --import-env /path/to/.env

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
log_ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

PROJECT="${DOPPLER_PROJECT:-logline-ecosystem}"
CONFIG="${DOPPLER_CONFIG:-dev}"
IMPORT_ENV_FILE=""

SERVICES=(
  "code247.logline.world"
  "edge-control.logline.world"
  "llm-gateway.logline.world"
  "logic.logline.world"
  "obs-api.logline.world"
)

usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --project <name>       Doppler project (default: ${PROJECT})
  --config <name>        Doppler config/environment (default: ${CONFIG})
  --import-env <path>    Import KEY=VALUE pairs from a local .env file
  -h, --help             Show this help
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project)
      PROJECT="$2"
      shift 2
      ;;
    --config)
      CONFIG="$2"
      shift 2
      ;;
    --import-env)
      IMPORT_ENV_FILE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      log_error "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

require_doppler() {
  if ! command -v doppler >/dev/null 2>&1; then
    log_error "Doppler CLI is not installed"
    echo "Install: brew install dopplerhq/cli/doppler"
    exit 1
  fi
  log_ok "Doppler CLI: $(doppler --version)"
}

require_auth() {
  if ! doppler whoami >/dev/null 2>&1; then
    log_error "Not authenticated in Doppler"
    echo "Run: doppler login"
    exit 1
  fi
  log_ok "Doppler authentication ok"
}

ensure_project() {
  if doppler projects get "$PROJECT" >/dev/null 2>&1; then
    log_ok "Project exists: $PROJECT"
    return
  fi

  log_info "Creating project: $PROJECT"
  doppler projects create "$PROJECT" --description "LogLine ecosystem secrets"
  log_ok "Project created: $PROJECT"
}

write_doppler_yaml() {
  local target_dir="$1"
  local target_file="$target_dir/doppler.yaml"

  cat > "$target_file" <<YAML
setup:
  project: ${PROJECT}
  config: ${CONFIG}
YAML

  log_ok "Wrote ${target_file#$ROOT_DIR/}"
}

setup_yaml_files() {
  write_doppler_yaml "$ROOT_DIR"

  for service in "${SERVICES[@]}"; do
    if [[ -d "$ROOT_DIR/$service" ]]; then
      write_doppler_yaml "$ROOT_DIR/$service"
    else
      log_warn "Service directory not found: $service"
    fi
  done
}

import_env_file() {
  local env_file="$1"
  if [[ ! -f "$env_file" ]]; then
    log_error "Env file not found: $env_file"
    exit 1
  fi

  log_info "Importing secrets from: $env_file"

  while IFS= read -r line || [[ -n "$line" ]]; do
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

    if [[ "$line" =~ ^([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
      key="${BASH_REMATCH[1]}"
      value="${BASH_REMATCH[2]}"
      [[ -z "$value" ]] && continue

      doppler secrets set "${key}=${value}" --project "$PROJECT" --config "$CONFIG" >/dev/null
      log_ok "Imported $key"
    fi
  done < "$env_file"
}

print_next_steps() {
  cat <<NEXT

Next steps:
  1) Fill required keys:
     ./scripts/doppler-secrets-audit.sh --project ${PROJECT} --config ${CONFIG}

  2) Start stack (PM2 will call doppler per service):
     pm2 start /Users/ubl-ops/Integration/ecosystem.config.cjs

  3) If PM2 is already running:
     pm2 restart all --update-env

Notes:
  - Secrets stay in Doppler; no .env file required for runtime.
  - You can still keep local non-secret defaults in app env blocks.
NEXT
}

main() {
  log_info "Bootstrapping Doppler for LogLine ecosystem"
  log_info "Project=${PROJECT} Config=${CONFIG}"

  require_doppler
  require_auth
  ensure_project
  setup_yaml_files

  if [[ -n "$IMPORT_ENV_FILE" ]]; then
    import_env_file "$IMPORT_ENV_FILE"
  fi

  print_next_steps
}

main "$@"
