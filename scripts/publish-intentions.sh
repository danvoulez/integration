#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <manifest-file> [ci-target] [meta-output-file]" >&2
  exit 1
fi

manifest_file="$1"
ci_target="${2:-""}"
meta_output="${3:-.code247/linear-meta.json}"
intentions_url="${CODE247_INTENTIONS_URL:-""}"
code247_token="${CODE247_TOKEN:-""}"

if [[ -z $intentions_url ]]; then
  intentions_url="https://code247.logline.world/intentions"
fi

if [[ -z $code247_token ]]; then
  echo "Set CODE247_TOKEN before running." >&2
  exit 1
fi

if [[ ! -f $manifest_file ]]; then
  echo "Manifest file $manifest_file not found." >&2
  exit 1
fi

manifest_data=$(jq -c . "$manifest_file")
body=$(jq -n --argjson manifest "$manifest_data" \
  --arg source "$PWD" \
  --arg revision "${CI_REVISION:-}" \
  --arg ci_target "$ci_target" \
  '{manifest: $manifest, source: $source, revision: $revision, ci_target: $ci_target}')

response_file=$(mktemp)
http_code=$(curl -sS -o "$response_file" -w "%{http_code}" -X POST "$intentions_url" \
  -H "Authorization: Bearer $code247_token" \
  -H "Content-Type: application/json" \
  -d "$body")

if [[ "$http_code" -lt 200 || "$http_code" -ge 300 ]]; then
  cat "$response_file"
  echo
  echo "Request failed with HTTP $http_code" >&2
  exit 1
fi

mkdir -p "$(dirname "$meta_output")"
jq . "$response_file" > "$meta_output"
cat "$meta_output"
