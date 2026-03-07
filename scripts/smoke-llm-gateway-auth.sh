#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATEWAY_DIR="${ROOT_DIR}/llm-gateway.logline.world"
ARTIFACTS_DIR="${ROOT_DIR}/artifacts"
ORIGINAL_HOME="${HOME:-/Users/ubl-ops}"
RUSTUP_HOME_DEFAULT="${RUSTUP_HOME:-${ORIGINAL_HOME}/.rustup}"
CARGO_HOME_DEFAULT="${CARGO_HOME:-${ORIGINAL_HOME}/.cargo}"
HOST="${LLM_GATEWAY_SMOKE_HOST:-127.0.0.1}"
PORT="${LLM_GATEWAY_SMOKE_PORT:-3301}"
BASE_URL=""
LOG_PATH="${LLM_GATEWAY_SMOKE_LOG_PATH:-${ARTIFACTS_DIR}/llm-gateway-auth-smoke.log}"
REPORT_PATH="${LLM_GATEWAY_SMOKE_REPORT_PATH:-${ARTIFACTS_DIR}/llm-gateway-auth-smoke-report.json}"
DB_PATH="${LLM_GATEWAY_SMOKE_DB_PATH:-${ARTIFACTS_DIR}/llm-gateway-auth-smoke.db}"
HOME_DIR="${LLM_GATEWAY_SMOKE_HOME_DIR:-${ARTIFACTS_DIR}/llm-gateway-auth-home}"
TENANT_ID="${LLM_GATEWAY_SMOKE_TENANT_ID:-${DEFAULT_TENANT_ID:-voulezvous}}"
APP_ID="${LLM_GATEWAY_SMOKE_APP_ID:-${DEFAULT_APP_ID:-code247}}"
ONBOARD_APP_NAME="${LLM_GATEWAY_SMOKE_LEGACY_APP_NAME:-llm-gateway-smoke-legacy}"
COMPAT_SUNSET_AT="${LLM_GATEWAY_SMOKE_COMPAT_SUNSET_AT:-2099-01-01T00:00:00Z}"
REQUIRED_SCOPE="${LLM_GATEWAY_SMOKE_REQUIRED_SCOPE:-${SUPABASE_REQUIRED_SERVICE_SCOPE:-llm:invoke}}"

mkdir -p "${ARTIFACTS_DIR}"
rm -f "${DB_PATH}"
rm -rf "${HOME_DIR}"
mkdir -p "${HOME_DIR}"
ln -sfn "${RUSTUP_HOME_DEFAULT}" "${HOME_DIR}/.rustup"
ln -sfn "${CARGO_HOME_DEFAULT}" "${HOME_DIR}/.cargo"

require_env() {
  local key="$1"
  if [[ -z "${!key:-}" ]]; then
    echo "missing required env: ${key}" >&2
    exit 1
  fi
}

require_env "SUPABASE_JWT_SECRET"
require_env "CLI_JWT_SECRET"
require_env "LLM_API_KEY"

if ! command -v node >/dev/null 2>&1; then
  echo "node is required" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi

SERVER_PID=""

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" >/dev/null 2>&1; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

make_hs256_jwt() {
  local secret="$1"
  local aud="$2"
  local payload_json="$3"
  node - "$secret" "$aud" "$payload_json" <<'EOF'
const crypto = require('crypto');

const [secret, audience, payloadJson] = process.argv.slice(2);
const now = Math.floor(Date.now() / 1000);

function base64url(input) {
  return Buffer.from(input)
    .toString('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
}

const header = { alg: 'HS256', typ: 'JWT' };
const payload = JSON.parse(payloadJson);
if (!payload.iat) payload.iat = now;
if (!payload.exp) payload.exp = now + 600;
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

make_service_jwt() {
  local scope="${1}"
  local aud="${SUPABASE_JWT_AUDIENCE:-}"
  make_hs256_jwt \
    "${SUPABASE_JWT_SECRET}" \
    "${aud}" \
    "{\"sub\":\"${APP_ID}\",\"role\":\"service\",\"tenant_id\":\"${TENANT_ID}\",\"scope\":\"${scope}\"}"
}

make_onboarding_jwt() {
  local aud="${CLI_JWT_AUDIENCE:-}"
  make_hs256_jwt \
    "${CLI_JWT_SECRET}" \
    "${aud}" \
    "{\"sub\":\"${ONBOARD_APP_NAME}\",\"app_name\":\"${ONBOARD_APP_NAME}\"}"
}

start_gateway() {
  local legacy_mode="$1"
  local sunset_at="$2"
  local log_suffix="$3"
  local port_offset="${4:-0}"
  local current_port=$((PORT + port_offset))

  cleanup
  local log_file="${LOG_PATH%.log}-${log_suffix}.log"
  : > "${log_file}"
  BASE_URL="http://${HOST}:${current_port}"

  (
    cd "${GATEWAY_DIR}"
    HOME="${HOME_DIR}" \
    PORT="${current_port}" \
    SUPABASE_REQUIRED_SERVICE_SCOPE="${REQUIRED_SCOPE}" \
    LLM_LEGACY_API_KEY_MODE="${legacy_mode}" \
    LLM_LEGACY_API_KEY_SUNSET_AT="${sunset_at}" \
    cargo run
  ) >"${log_file}" 2>&1 &
  SERVER_PID="$!"

  for _ in $(seq 1 60); do
    if curl -fsS "${BASE_URL}/health" >/dev/null 2>&1; then
      cp "${log_file}" "${LOG_PATH}"
      return 0
    fi
    sleep 1
  done

  cp "${log_file}" "${LOG_PATH}" || true
  echo "llm-gateway did not become healthy; see ${log_file}" >&2
  exit 1
}

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

onboard_legacy_key() {
  local onboarding_token="$1"
  local body
  body="{\"app_name\":\"${ONBOARD_APP_NAME}\",\"rotate\":true}"
  read -r status body_file <<<"$(request POST "${onboarding_token}" "/v1/onboarding/sync" "${body}")"
  assert_status "${status}" "200" "${body_file}"
  assert_success_body "${body_file}"
  node -e 'const fs=require("fs"); const data=JSON.parse(fs.readFileSync(process.argv[1],"utf8")); process.stdout.write(data.api_key);' "${body_file}"
}

batch_body='{"custom_id":"auth-smoke","provider":"openai","model":"gpt-4o-mini","messages":[{"role":"user","content":"auth smoke"}],"max_tokens":32}'

echo "[llm-gateway-auth-smoke] log=${LOG_PATH}"
echo "[llm-gateway-auth-smoke] report=${REPORT_PATH}"

JWT_TOKEN="$(make_service_jwt "${REQUIRED_SCOPE}")"
ONBOARD_TOKEN="$(make_onboarding_jwt)"

start_gateway "compat" "${COMPAT_SUNSET_AT}" "compat" 0
echo "[llm-gateway-auth-smoke] compat_base_url=${BASE_URL}"
LEGACY_TOKEN="$(onboard_legacy_key "${ONBOARD_TOKEN}")"

read -r jwt_batch_status jwt_batch_body <<<"$(request POST "${JWT_TOKEN}" "/v1/batch" "${batch_body}")"
assert_status "${jwt_batch_status}" "200" "${jwt_batch_body}"
assert_success_body "${jwt_batch_body}"
JWT_JOB_ID="$(node -e 'const fs=require("fs"); const data=JSON.parse(fs.readFileSync(process.argv[1],"utf8")); process.stdout.write(data.job_id);' "${jwt_batch_body}")"

read -r jwt_batch_lookup_status jwt_batch_lookup_body <<<"$(request GET "${JWT_TOKEN}" "/v1/batch/${JWT_JOB_ID}")"
assert_status "${jwt_batch_lookup_status}" "200" "${jwt_batch_lookup_body}"
assert_success_body "${jwt_batch_lookup_body}"

read -r legacy_batch_status legacy_batch_body <<<"$(request POST "${LEGACY_TOKEN}" "/v1/batch" "${batch_body}")"
assert_status "${legacy_batch_status}" "200" "${legacy_batch_body}"
assert_success_body "${legacy_batch_body}"

start_gateway "disabled" "${COMPAT_SUNSET_AT}" "jwt-only" 1
echo "[llm-gateway-auth-smoke] disabled_base_url=${BASE_URL}"

read -r legacy_disabled_status legacy_disabled_body <<<"$(request POST "${LEGACY_TOKEN}" "/v1/batch" "${batch_body}")"
assert_status "${legacy_disabled_status}" "401" "${legacy_disabled_body}"

read -r jwt_disabled_status jwt_disabled_body <<<"$(request POST "${JWT_TOKEN}" "/v1/batch" "${batch_body}")"
assert_status "${jwt_disabled_status}" "200" "${jwt_disabled_body}"
assert_success_body "${jwt_disabled_body}"

cat >"${REPORT_PATH}" <<EOF
{
  "report_version": "llm-gateway.auth-smoke.v1",
  "base_url": "${BASE_URL}",
  "tenant_id": "${TENANT_ID}",
  "app_id": "${APP_ID}",
  "results": {
    "jwt_batch_submit_compat": ${jwt_batch_status},
    "jwt_batch_lookup_compat": ${jwt_batch_lookup_status},
    "legacy_batch_submit_compat": ${legacy_batch_status},
    "legacy_batch_submit_disabled": ${legacy_disabled_status},
    "jwt_batch_submit_disabled": ${jwt_disabled_status}
  }
}
EOF

echo "[llm-gateway-auth-smoke] ok"
echo "[llm-gateway-auth-smoke] report=${REPORT_PATH}"
