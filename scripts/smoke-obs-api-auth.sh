#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OBS_API_DIR="${ROOT_DIR}/obs-api.logline.world"
ARTIFACTS_DIR="${ROOT_DIR}/artifacts"
HOST="${OBS_API_SMOKE_HOST:-127.0.0.1}"
PORT="${OBS_API_SMOKE_PORT:-3101}"
BASE_URL="http://${HOST}:${PORT}"
TENANT_ID="${OBS_API_SMOKE_TENANT_ID:-${DEFAULT_TENANT_ID:-voulezvous}}"
APP_ID="${OBS_API_SMOKE_APP_ID:-${DEFAULT_APP_ID:-code247}}"
LOG_PATH="${OBS_API_SMOKE_LOG_PATH:-${ARTIFACTS_DIR}/obs-api-auth-smoke.log}"
REPORT_PATH="${OBS_API_SMOKE_REPORT_PATH:-${ARTIFACTS_DIR}/obs-api-auth-smoke-report.json}"

mkdir -p "${ARTIFACTS_DIR}"

require_env() {
  local key="$1"
  if [[ -z "${!key:-}" ]]; then
    echo "missing required env: ${key}" >&2
    exit 1
  fi
}

require_env "SUPABASE_JWT_SECRET"
require_env "NEXT_PUBLIC_SUPABASE_URL"
require_env "NEXT_PUBLIC_SUPABASE_ANON_KEY"

if [[ -z "${SUPABASE_DB_URL:-}" && -z "${DATABASE_URL:-}" && -z "${DATABASE_URL_UNPOOLED:-}" && -z "${POSTGRES_URL:-}" ]]; then
  echo "missing database env: one of SUPABASE_DB_URL, DATABASE_URL, DATABASE_URL_UNPOOLED or POSTGRES_URL is required" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node is required" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi

make_jwt() {
  local scope="$1"
  node - "$SUPABASE_JWT_SECRET" "$scope" "${OBS_API_SMOKE_SUB:-obs-api-smoke}" "${SUPABASE_JWT_ISSUER:-}" "${SUPABASE_JWT_AUDIENCE:-}" "$TENANT_ID" "$APP_ID" <<'EOF'
const crypto = require('crypto');

const [secret, scope, sub, issuer, audience, tenantId, appId] = process.argv.slice(2);
const now = Math.floor(Date.now() / 1000);

function base64url(input) {
  return Buffer.from(input)
    .toString('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
}

const header = { alg: 'HS256', typ: 'JWT' };
const payload = {
  sub,
  role: 'service',
  scope,
  tenant_id: tenantId,
  app_id: appId,
  iat: now,
  exp: now + 600,
};

if (issuer) payload.iss = issuer;
if (audience) payload.aud = audience;

const encodedHeader = base64url(JSON.stringify(header));
const encodedPayload = base64url(JSON.stringify(payload));
const signingInput = `${encodedHeader}.${encodedPayload}`;
const signature = crypto
  .createHmac('sha256', secret)
  .update(signingInput)
  .digest('base64')
  .replace(/=/g, '')
  .replace(/\+/g, '-')
  .replace(/\//g, '_');

process.stdout.write(`${signingInput}.${signature}`);
EOF
}

SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "[obs-api-auth-smoke] log=${LOG_PATH}"
echo "[obs-api-auth-smoke] report=${REPORT_PATH}"
echo "[obs-api-auth-smoke] base_url=${BASE_URL}"

(
  cd "${OBS_API_DIR}"
  npm run dev -- --hostname "${HOST}" --port "${PORT}"
) >"${LOG_PATH}" 2>&1 &
SERVER_PID="$!"

for _ in $(seq 1 60); do
  if curl -fsS "${BASE_URL}/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! curl -fsS "${BASE_URL}/api/health" >/dev/null 2>&1; then
  echo "obs-api did not become healthy; see ${LOG_PATH}" >&2
  exit 1
fi

READ_TOKEN="$(make_jwt 'obs:read')"
ACK_TOKEN="$(make_jwt 'obs:alerts:ack')"
BAD_TOKEN="$(make_jwt 'code247:jobs:read')"

request() {
  local method="$1"
  local token="$2"
  local path="$3"
  local body="${4:-}"
  local output_file
  output_file="$(mktemp)"
  local http_code

  if [[ -n "${body}" ]]; then
    http_code="$(curl -sS -o "${output_file}" -w '%{http_code}' \
      -X "${method}" \
      -H "authorization: Bearer ${token}" \
      -H 'content-type: application/json' \
      "${BASE_URL}${path}" \
      -d "${body}")"
  else
    http_code="$(curl -sS -o "${output_file}" -w '%{http_code}' \
      -X "${method}" \
      -H "authorization: Bearer ${token}" \
      "${BASE_URL}${path}")"
  fi

  printf '%s %s\n' "${http_code}" "${output_file}"
}

assert_status() {
  local actual="$1"
  local expected="$2"
  local body_file="$3"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "unexpected status: got=${actual} expected=${expected}" >&2
    cat "${body_file}" >&2
    exit 1
  fi
}

assert_success_body() {
  local body_file="$1"
  if ! grep -q '"request_id"[[:space:]]*:' "${body_file}" || ! grep -q '"output_schema"[[:space:]]*:[[:space:]]*"https://logline.world/schemas/response-envelope.v1.schema.json"' "${body_file}"; then
    echo "expected success envelope" >&2
    cat "${body_file}" >&2
    exit 1
  fi
}

dashboard_path="/api/v1/fuel/dashboard?tenant_id=${TENANT_ID}&app_id=${APP_ID}"
alerts_path="/api/v1/fuel/alerts?tenant_id=${TENANT_ID}&app_id=${APP_ID}"
calibration_path="/api/v1/fuel/calibration?tenant_id=${TENANT_ID}&app_id=${APP_ID}&days=14"
reconciliation_path="/api/v1/fuel/reconciliation?tenant_id=${TENANT_ID}&app_id=${APP_ID}&days=14"
ops_get_path="/api/v1/fuel/ops?tenant_id=${TENANT_ID}&app_id=${APP_ID}&days=14"
ops_post_path="/api/v1/fuel/ops"
ops_post_body='{"job_name":"baseline_and_alerts"}'
code247_stage_telemetry_path="/api/v1/code247/stage-telemetry?tenant_id=${TENANT_ID}&app_id=code247&days=14"
code247_run_timeline_path="/api/v1/code247/run-timeline?tenant_id=${TENANT_ID}&app_id=code247&days=14&jobs_limit=10&limit=100"

read -r unauthorized_status unauthorized_body <<<"$(request GET "${BAD_TOKEN}" "${dashboard_path}")"
assert_status "${unauthorized_status}" "403" "${unauthorized_body}"

read -r dashboard_status dashboard_body <<<"$(request GET "${READ_TOKEN}" "${dashboard_path}")"
assert_status "${dashboard_status}" "200" "${dashboard_body}"
assert_success_body "${dashboard_body}"

read -r alerts_status alerts_body <<<"$(request GET "${READ_TOKEN}" "${alerts_path}")"
assert_status "${alerts_status}" "200" "${alerts_body}"
assert_success_body "${alerts_body}"

read -r calibration_status calibration_body <<<"$(request GET "${READ_TOKEN}" "${calibration_path}")"
assert_status "${calibration_status}" "200" "${calibration_body}"
assert_success_body "${calibration_body}"

read -r reconciliation_status reconciliation_body <<<"$(request GET "${READ_TOKEN}" "${reconciliation_path}")"
assert_status "${reconciliation_status}" "200" "${reconciliation_body}"
assert_success_body "${reconciliation_body}"

read -r ops_get_status ops_get_body <<<"$(request GET "${READ_TOKEN}" "${ops_get_path}")"
assert_status "${ops_get_status}" "200" "${ops_get_body}"
assert_success_body "${ops_get_body}"

read -r code247_stage_telemetry_status code247_stage_telemetry_body <<<"$(request GET "${READ_TOKEN}" "${code247_stage_telemetry_path}")"
assert_status "${code247_stage_telemetry_status}" "200" "${code247_stage_telemetry_body}"
assert_success_body "${code247_stage_telemetry_body}"

read -r code247_run_timeline_status code247_run_timeline_body <<<"$(request GET "${READ_TOKEN}" "${code247_run_timeline_path}")"
assert_status "${code247_run_timeline_status}" "200" "${code247_run_timeline_body}"
assert_success_body "${code247_run_timeline_body}"

read -r ops_post_forbidden_status ops_post_forbidden_body <<<"$(request POST "${READ_TOKEN}" "${ops_post_path}" "${ops_post_body}")"
assert_status "${ops_post_forbidden_status}" "403" "${ops_post_forbidden_body}"

read -r ops_post_status ops_post_body_file <<<"$(request POST "${ACK_TOKEN}" "${ops_post_path}" "${ops_post_body}")"
assert_status "${ops_post_status}" "200" "${ops_post_body_file}"
assert_success_body "${ops_post_body_file}"

cat >"${REPORT_PATH}" <<EOF
{
  "report_version": "obs-api.auth-smoke.v1",
  "base_url": "${BASE_URL}",
  "tenant_id": "${TENANT_ID}",
  "app_id": "${APP_ID}",
  "results": {
    "dashboard_forbidden_without_scope": ${unauthorized_status},
    "fuel_dashboard": ${dashboard_status},
    "fuel_alerts": ${alerts_status},
    "fuel_calibration": ${calibration_status},
    "fuel_reconciliation": ${reconciliation_status},
    "fuel_ops_get": ${ops_get_status},
    "code247_stage_telemetry": ${code247_stage_telemetry_status},
    "code247_run_timeline": ${code247_run_timeline_status},
    "fuel_ops_post_forbidden_without_ack_scope": ${ops_post_forbidden_status},
    "fuel_ops_post": ${ops_post_status}
  }
}
EOF

rm -f \
  "${unauthorized_body}" \
  "${dashboard_body}" \
  "${alerts_body}" \
  "${calibration_body}" \
  "${reconciliation_body}" \
  "${ops_get_body}" \
  "${code247_stage_telemetry_body}" \
  "${code247_run_timeline_body}" \
  "${ops_post_forbidden_body}" \
  "${ops_post_body_file}"

echo "[obs-api-auth-smoke] ok"
