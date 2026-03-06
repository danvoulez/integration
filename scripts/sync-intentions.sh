#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <sync-payload-file> [meta-output-file]" >&2
  exit 1
fi

payload_file="$1"
meta_output="${2:-.code247/linear-meta.json}"
sync_url="${CODE247_INTENTIONS_SYNC_URL:-https://code247.logline.world/intentions/sync}"
code247_token="${CODE247_TOKEN:-""}"

if [[ -z $code247_token ]]; then
  echo "Set CODE247_TOKEN before running." >&2
  exit 1
fi

if [[ ! -f $payload_file ]]; then
  echo "Payload file $payload_file not found." >&2
  exit 1
fi

response_file=$(mktemp)
http_code=$(curl -sS -o "$response_file" -w "%{http_code}" -X POST "$sync_url" \
  -H "Authorization: Bearer $code247_token" \
  -H "Content-Type: application/json" \
  -d "@$payload_file")

if [[ "$http_code" -lt 200 || "$http_code" -ge 300 ]]; then
  cat "$response_file"
  echo
  echo "Sync request failed with HTTP $http_code" >&2
  exit 1
fi

mkdir -p "$(dirname "$meta_output")"
if [[ -f "$meta_output" ]] && jq -e . "$meta_output" >/dev/null 2>&1; then
  jq -n --argjson current "$(cat "$meta_output")" --argjson sync "$(cat "$response_file")" \
    '$current + {sync: $sync}' > "$meta_output"
else
  jq -n --argjson sync "$(cat "$response_file")" '{sync: $sync}' > "$meta_output"
fi

jq . "$response_file"
