#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <manifest-file> <intention-id> [workspace] [project]" >&2
  exit 1
fi

manifest_file="$1"
intention_id="$2"
workspace_override="${3:-}"
project_override="${4:-}"

if [[ ! -f "$manifest_file" ]]; then
  echo "Manifest file not found: $manifest_file" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for this smoke script." >&2
  exit 1
fi

code247_token="${CODE247_TOKEN:-${CODE247_INTENTIONS_TOKEN:-}}"
if [[ -z "$code247_token" ]]; then
  echo "Set CODE247_TOKEN (or CODE247_INTENTIONS_TOKEN) before running." >&2
  exit 1
fi

base_url="${CODE247_BASE_URL:-http://127.0.0.1:4001}"
intentions_url="${CODE247_INTENTIONS_URL:-$base_url/intentions}"
sync_url="${CODE247_INTENTIONS_SYNC_URL:-$base_url/intentions/sync}"

manifest_workspace="$(jq -r '.workspace // empty' "$manifest_file")"
manifest_project="$(jq -r '.project // empty' "$manifest_file")"

workspace="${workspace_override:-$manifest_workspace}"
project="${project_override:-$manifest_project}"

if [[ -z "$workspace" || -z "$project" ]]; then
  echo "workspace/project missing. Provide args or ensure manifest has both fields." >&2
  exit 1
fi

manifest_compact="$(jq -c . "$manifest_file")"
intake_body="$(
  jq -n \
    --argjson manifest "$manifest_compact" \
    --arg source "$PWD" \
    --arg revision "smoke-p1" \
    '{manifest: $manifest, source: $source, revision: $revision}'
)"

echo "[smoke] intake: $intentions_url"
intake_resp="$(mktemp)"
intake_http="$(
  curl -sS -o "$intake_resp" -w "%{http_code}" \
    -X POST "$intentions_url" \
    -H "Authorization: Bearer $code247_token" \
    -H "Content-Type: application/json" \
    -d "$intake_body"
)"
if [[ "$intake_http" -lt 200 || "$intake_http" -ge 300 ]]; then
  cat "$intake_resp"
  echo
  echo "Intake failed with HTTP $intake_http" >&2
  exit 1
fi

invalid_done_payload="$(mktemp)"
jq -n \
  --arg workspace "$workspace" \
  --arg project "$project" \
  --arg intention_id "$intention_id" \
  '{
    workspace: $workspace,
    project: $project,
    results: [
      {
        intention_id: $intention_id,
        status: "success",
        summary: "smoke: force invalid Done transition",
        set_done_on_success: true,
        ci: {
          queue_id: "ci-smoke-p1",
          job: "job-smoke-p1",
          url: "https://ci.example/smoke"
        },
        evidence: [
          {label: "deploy", url: "https://deploy.example/smoke"}
        ]
      }
    ]
  }' > "$invalid_done_payload"

echo "[smoke] sync (expect INVALID_STATE_TRANSITION): $sync_url"
sync_resp="$(mktemp)"
sync_http="$(
  curl -sS -o "$sync_resp" -w "%{http_code}" \
    -X POST "$sync_url" \
    -H "Authorization: Bearer $code247_token" \
    -H "Content-Type: application/json" \
    -d "@$invalid_done_payload"
)"
if [[ "$sync_http" -lt 200 || "$sync_http" -ge 300 ]]; then
  cat "$sync_resp"
  echo
  echo "Sync failed with HTTP $sync_http" >&2
  exit 1
fi

if ! jq -e \
  --arg intention_id "$intention_id" \
  '.errors[]? | select(.intention_id == $intention_id and .code == "INVALID_STATE_TRANSITION")' \
  "$sync_resp" >/dev/null; then
  echo "Expected INVALID_STATE_TRANSITION for intention '$intention_id', but it was not found." >&2
  cat "$sync_resp"
  exit 1
fi

echo "[smoke] PASS: invalid Done transition was blocked."
jq . "$sync_resp"

positive_sync_payload="$(mktemp)"
jq -n \
  --arg workspace "$workspace" \
  --arg project "$project" \
  --arg intention_id "$intention_id" \
  '{
    workspace: $workspace,
    project: $project,
    results: [
      {
        intention_id: $intention_id,
        status: "failed",
        summary: "smoke: comment-only sync should succeed without transition"
      }
    ]
  }' > "$positive_sync_payload"

echo "[smoke] sync (expect success without transition): $sync_url"
positive_resp="$(mktemp)"
positive_http="$(
  curl -sS -o "$positive_resp" -w "%{http_code}" \
    -X POST "$sync_url" \
    -H "Authorization: Bearer $code247_token" \
    -H "Content-Type: application/json" \
    -d "@$positive_sync_payload"
)"
if [[ "$positive_http" -lt 200 || "$positive_http" -ge 300 ]]; then
  cat "$positive_resp"
  echo
  echo "Positive sync failed with HTTP $positive_http" >&2
  exit 1
fi

if ! jq -e --arg intention_id "$intention_id" \
  '.synced[]? | select(.intention_id == $intention_id)' \
  "$positive_resp" >/dev/null; then
  echo "Expected synced entry for intention '$intention_id' in positive flow." >&2
  cat "$positive_resp"
  exit 1
fi

echo "[smoke] PASS: positive comment-only sync completed."
jq . "$positive_resp"
