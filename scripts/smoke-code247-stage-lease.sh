#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_PATH="${CODE247_DB_PATH:-${ROOT_DIR}/code247.logline.world/dual_agents.db}"
STAGE="${STAGE:-CODING}"
WAIT_SECONDS="${WAIT_SECONDS:-25}"
POLL_INTERVAL_SECONDS="${POLL_INTERVAL_SECONDS:-1}"

if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "[stage-lease-smoke] sqlite3 not found" >&2
  exit 1
fi

if [[ ! -f "${DB_PATH}" ]]; then
  echo "[stage-lease-smoke] db not found: ${DB_PATH}" >&2
  exit 1
fi

JOB_ID="smoke-stage-lease-$(date -u +%s)-$$"
ISSUE_ID="smoke:stage-lease:${JOB_ID}"
PAYLOAD='{"smoke_test":true,"source":"stage-lease-smoke"}'

echo "[stage-lease-smoke] db=${DB_PATH}"
echo "[stage-lease-smoke] job_id=${JOB_ID}"
echo "[stage-lease-smoke] stage=${STAGE}"

sqlite3 "${DB_PATH}" <<SQL
INSERT INTO jobs (
  id,
  issue_id,
  status,
  payload,
  retries,
  last_error,
  stage_started_at,
  heartbeat_at,
  lease_expires_at,
  lease_owner,
  stage_attempt,
  created_at,
  updated_at
) VALUES (
  '${JOB_ID}',
  '${ISSUE_ID}',
  '${STAGE}',
  '${PAYLOAD}',
  0,
  NULL,
  STRFTIME('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 minutes'),
  STRFTIME('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 minutes'),
  STRFTIME('%Y-%m-%dT%H:%M:%SZ', 'now', '-2 minutes'),
  'smoke-runner',
  1,
  STRFTIME('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 minutes'),
  STRFTIME('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 minutes')
);
SQL

deadline=$((SECONDS + WAIT_SECONDS))
while (( SECONDS < deadline )); do
  status="$(sqlite3 "${DB_PATH}" "SELECT status FROM jobs WHERE id='${JOB_ID}'")"
  last_error="$(sqlite3 "${DB_PATH}" "SELECT COALESCE(last_error, '') FROM jobs WHERE id='${JOB_ID}'")"
  lease_events="$(sqlite3 "${DB_PATH}" "SELECT COUNT(1) FROM execution_log WHERE job_id='${JOB_ID}' AND stage='lease_expired'")"

  if [[ "${lease_events}" -gt 1 ]]; then
    echo "[stage-lease-smoke] duplicated lease_expired events for ${JOB_ID}" >&2
    exit 1
  fi

  if [[ "${status}" == "FAILED" && "${lease_events}" == "1" ]]; then
    echo "[stage-lease-smoke] ok"
    echo "[stage-lease-smoke] status=${status}"
    echo "[stage-lease-smoke] lease_expired_events=${lease_events}"
    echo "[stage-lease-smoke] last_error=${last_error}"
    exit 0
  fi

  sleep "${POLL_INTERVAL_SECONDS}"
done

status="$(sqlite3 "${DB_PATH}" "SELECT status FROM jobs WHERE id='${JOB_ID}'")"
last_error="$(sqlite3 "${DB_PATH}" "SELECT COALESCE(last_error, '') FROM jobs WHERE id='${JOB_ID}'")"
lease_events="$(sqlite3 "${DB_PATH}" "SELECT COUNT(1) FROM execution_log WHERE job_id='${JOB_ID}' AND stage='lease_expired'")"

echo "[stage-lease-smoke] timeout waiting for lease expiration handling" >&2
echo "[stage-lease-smoke] status=${status}" >&2
echo "[stage-lease-smoke] lease_expired_events=${lease_events}" >&2
echo "[stage-lease-smoke] last_error=${last_error}" >&2
exit 1
