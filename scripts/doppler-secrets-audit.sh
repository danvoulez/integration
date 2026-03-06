#!/usr/bin/env bash
set -euo pipefail

# Audit required/optional secrets in Doppler against manifest.
#
# Usage:
#   ./scripts/doppler-secrets-audit.sh
#   ./scripts/doppler-secrets-audit.sh --all
#   ./scripts/doppler-secrets-audit.sh --project logline-ecosystem --config dev

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
MANIFEST_FILE="$ROOT_DIR/secrets/doppler-secrets-manifest.tsv"

PROJECT="${DOPPLER_PROJECT:-logline-ecosystem}"
CONFIG="${DOPPLER_CONFIG:-dev}"
CHECK_ALL=0

usage() {
  cat <<USAGE
Usage: $0 [options]

Options:
  --project <name>   Doppler project (default: ${PROJECT})
  --config <name>    Doppler config/environment (default: ${CONFIG})
  --all              Check optional keys too
  -h, --help         Show help
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
    --all)
      CHECK_ALL=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if ! command -v doppler >/dev/null 2>&1; then
  echo "ERROR: Doppler CLI not installed" >&2
  exit 1
fi

if ! doppler whoami >/dev/null 2>&1; then
  echo "ERROR: run 'doppler login' first" >&2
  exit 1
fi

if [[ ! -f "$MANIFEST_FILE" ]]; then
  echo "ERROR: manifest file not found: $MANIFEST_FILE" >&2
  exit 1
fi

missing_required=()
missing_optional=()
present_count=0
checked_count=0

add_missing_required_once() {
  local key="$1"
  for existing in "${missing_required[@]}"; do
    if [[ "$existing" == "$key" ]]; then
      return
    fi
  done
  missing_required+=("$key")
}

while IFS=$'\t' read -r key required services notes; do
  [[ -z "${key:-}" || "${key:0:1}" == "#" ]] && continue

  if [[ "$required" != "required" && "$CHECK_ALL" -eq 0 ]]; then
    continue
  fi

  checked_count=$((checked_count + 1))
  if doppler secrets get "$key" --project "$PROJECT" --config "$CONFIG" --plain >/dev/null 2>&1; then
    present_count=$((present_count + 1))
    echo "[OK]      $key"
  else
    if [[ "$required" == "required" ]]; then
      missing_required+=("$key")
      echo "[MISSING] $key (required)"
    else
      missing_optional+=("$key")
      echo "[MISSING] $key (optional)"
    fi
  fi
done < "$MANIFEST_FILE"

has_secret() {
  local key="$1"
  doppler secrets get "$key" --project "$PROJECT" --config "$CONFIG" --plain >/dev/null 2>&1
}

# Composite policy checks (mode-based requirements)
if has_secret LINEAR_TEAM_ID; then
  if has_secret LINEAR_API_KEY; then
    echo "[OK]      LINEAR auth mode = API key"
  elif has_secret LINEAR_CLIENT_ID && has_secret LINEAR_CLIENT_SECRET; then
    echo "[OK]      LINEAR auth mode = OAuth client"
  else
    add_missing_required_once "LINEAR_AUTH_MODE (set LINEAR_API_KEY or LINEAR_CLIENT_ID+LINEAR_CLIENT_SECRET)"
    echo "[MISSING] LINEAR auth mode (required)"
  fi
else
  add_missing_required_once "LINEAR_TEAM_ID"
  echo "[MISSING] LINEAR_TEAM_ID (required)"
fi

if has_secret GITHUB_TOKEN; then
  echo "[OK]      GitHub auth mode = token"
elif has_secret GITHUB_APP_ID && has_secret GITHUB_APP_PRIVATE_KEY && has_secret GITHUB_APP_INSTALLATION_ID; then
  add_missing_required_once "GITHUB_TOKEN"
  echo "[MISSING] GitHub token mode (required by current code247 runtime)"
  echo "[INFO]    GitHub App keys detected; app-mode wiring can be enabled in runtime later"
else
  add_missing_required_once "GITHUB_TOKEN"
  echo "[MISSING] GITHUB_TOKEN (required by current code247 runtime)"
fi

echo ""
echo "Audit summary"
echo "  Project: $PROJECT"
echo "  Config:  $CONFIG"
echo "  Checked: $checked_count"
echo "  Present: $present_count"
echo ""

if [[ ${#missing_required[@]} -gt 0 ]]; then
  echo "Missing required keys (${#missing_required[@]}):"
  for key in "${missing_required[@]}"; do
    echo "  - $key"
  done
  echo ""
  echo "Set keys with:"
  if [[ "${missing_required[0]}" == *"("* ]]; then
    echo "  doppler secrets set GITHUB_TOKEN=... --project $PROJECT --config $CONFIG"
    echo "  # or use GitHub App keys (GITHUB_APP_ID, GITHUB_APP_PRIVATE_KEY, GITHUB_APP_INSTALLATION_ID)"
  else
    echo "  doppler secrets set ${missing_required[0]}=... --project $PROJECT --config $CONFIG"
  fi
  exit 2
fi

if [[ "$CHECK_ALL" -eq 1 && ${#missing_optional[@]} -gt 0 ]]; then
  echo "Missing optional keys (${#missing_optional[@]}):"
  for key in "${missing_optional[@]}"; do
    echo "  - $key"
  done
  echo ""
fi

echo "All required keys are present."
