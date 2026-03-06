#!/bin/bash
# LLM Gateway Refactoring Script
# Splits main.rs into separate modules

set -e
cd /Users/ubl-ops/Integration/llm-gateway.logline.world/src

# Backup original
cp main.rs main.rs.backup

echo "=== Creating module files ==="

# ─────────────────────────────────────────────────────────────────────────────
# config.rs: Lines 1-595 (imports + all config structs)
# ─────────────────────────────────────────────────────────────────────────────
cat > config.rs << 'CONFIGEOF'
//! Configuration loading and structs for the LLM Gateway.

use serde::Deserialize;
use std::path::PathBuf;
use tracing::{info, warn};

CONFIGEOF
sed -n '33,595p' main.rs >> config.rs
echo "" >> config.rs
echo "pub fn maybe_redact_upstream_url(config: &Config, upstream_url: &str) -> String {" >> config.rs
echo '    if config.security.expose_upstream_url { upstream_url.to_string() } else { "redacted".into() }' >> config.rs
echo "}" >> config.rs

# ─────────────────────────────────────────────────────────────────────────────
# fuel.rs: Lines 596-873 (fuel DB, Supabase emit)
# ─────────────────────────────────────────────────────────────────────────────
cat > fuel.rs << 'FUELEOF'
//! Fuel tracking and billing: local SQLite + Supabase integration.

use crate::config::{Config, SupabaseConfig};
use crate::types::{ChatMessage, FuelDelta};
use rusqlite::OptionalExtension;
use serde_json::json;
use std::path::{Path, PathBuf};
use tracing::warn;

FUELEOF
sed -n '596,873p' main.rs >> fuel.rs

# ─────────────────────────────────────────────────────────────────────────────
# qc.rs: Lines 874-940 (QC sampling)
# ─────────────────────────────────────────────────────────────────────────────
cat > qc.rs << 'QCEOF'
//! Quality Control sampling for LLM requests.

use crate::config::QcPolicy;
use crate::types::ChatRequest;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

QCEOF
sed -n '874,940p' main.rs >> qc.rs

# ─────────────────────────────────────────────────────────────────────────────
# auth.rs: Lines 941-1414 (auth, client identity, JWT)
# ─────────────────────────────────────────────────────────────────────────────
cat > auth.rs << 'AUTHEOF'
//! Authentication: API keys, Supabase JWT, client identity.

use crate::config::{Config, SupabaseConfig};
use crate::types::{AppState, ErrorResponse};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use std::sync::Arc;
use tracing::warn;

AUTHEOF
sed -n '941,1414p' main.rs >> auth.rs

# ─────────────────────────────────────────────────────────────────────────────
# types.rs: Lines 1416-1645 (API types, state, routing types)
# ─────────────────────────────────────────────────────────────────────────────
cat > types.rs << 'TYPESEOF'
//! API request/response types for the LLM Gateway.

use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

TYPESEOF
sed -n '1416,1645p' main.rs >> types.rs

# Add token estimation (3048-3070) and error_response (4145-4159) to types.rs
echo "" >> types.rs
echo "// Token estimation" >> types.rs
sed -n '3048,3070p' main.rs >> types.rs
echo "" >> types.rs
echo "// Error helper" >> types.rs
sed -n '4145,4159p' main.rs >> types.rs

# ─────────────────────────────────────────────────────────────────────────────
# handlers.rs: Lines 1647-1895, 3070-3164 (HTTP handlers, response helpers)
# ─────────────────────────────────────────────────────────────────────────────
cat > handlers.rs << 'HANDLERSEOF'
//! HTTP request handlers for the LLM Gateway API.

use crate::auth::{authenticate_client, ClientIdentity};
use crate::config::Config;
use crate::fuel::{emit_fuel_to_supabase, log_llm_request_to_supabase, upsert_daily_fuel};
use crate::qc::{insert_qc_sample, QcSampleRow, should_sample_qc, qc_request_key, redact_qc_text};
use crate::routing::{build_route_candidates, call_provider_candidate_with_retry, execution_budget, mark_candidate_success, mark_candidate_failure};
use crate::streaming::stream_chat_completions;
use crate::types::*;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tracing::warn;

HANDLERSEOF
# Handlers: health, list_models, matrix, metrics, fuel, fuel_daily, qc_samples, onboarding_sync, admin_daily_client_usage
sed -n '1647,1895p' main.rs >> handlers.rs

# Response helpers (streaming_chat_response, chunk_text)
echo "" >> handlers.rs
echo "// Response helpers" >> handlers.rs
sed -n '3070,3164p' main.rs >> handlers.rs

# chat_completions handler (1897-2355) - main request handler
echo "" >> handlers.rs
echo "// Main chat completions handler" >> handlers.rs
sed -n '1897,2355p' main.rs >> handlers.rs

# ─────────────────────────────────────────────────────────────────────────────
# streaming.rs: Lines 2357-3046 (SSE streaming)
# ─────────────────────────────────────────────────────────────────────────────
cat > streaming.rs << 'STREAMEOF'
//! Server-Sent Events (SSE) streaming for chat completions.

use crate::auth::ClientIdentity;
use crate::config::Config;
use crate::fuel::{emit_fuel_to_supabase, log_llm_request_to_supabase, upsert_daily_fuel};
use crate::routing::{build_route_candidates, execution_budget, mark_candidate_success, mark_candidate_failure, increment_provider_selected, candidate_key};
use crate::types::*;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use futures_util::StreamExt;
use serde_json::json;
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio_stream::wrappers::ReceiverStream;
use tracing::warn;

STREAMEOF
sed -n '2357,3046p' main.rs >> streaming.rs

# ─────────────────────────────────────────────────────────────────────────────
# routing.rs: Lines 3166-3580, 3752-3889, 4161-4293 (routing logic, circuit breaker, scoring)
# ─────────────────────────────────────────────────────────────────────────────
cat > routing.rs << 'ROUTINGEOF'
//! Request routing: candidate selection, circuit breaker, model scoring.

use crate::config::{Config, LlmRoute, ProviderConfig, ReliabilityPolicy};
use crate::types::*;
use axum::http::StatusCode;
use axum::Json;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

ROUTINGEOF
# Routing candidate building
sed -n '3166,3252p' main.rs >> routing.rs
# local_candidates, premium_provider_order, premium_candidates, genius/fast/code_candidates
sed -n '3253,3485p' main.rs >> routing.rs
# execution_budget, candidate_key, observe_request_latency, increment_provider_selected
sed -n '3486,3518p' main.rs >> routing.rs
# Circuit breaker
sed -n '3519,3582p' main.rs >> routing.rs
# call_provider_candidate_with_retry, call_provider_candidate
echo "" >> routing.rs
echo "// Provider candidate calls with retry" >> routing.rs
sed -n '3584,3750p' main.rs >> routing.rs
# validate_gateway_auth, parse_mode, route_mode_name, classify_task, choose_local*, provider_tuple, provider_ready, task_name
echo "" >> routing.rs
echo "// Helpers" >> routing.rs
sed -n '3752,3889p' main.rs >> routing.rs
# Model scoring
echo "" >> routing.rs
echo "// Model scoring" >> routing.rs
sed -n '4161,4293p' main.rs >> routing.rs

# ─────────────────────────────────────────────────────────────────────────────
# providers/mod.rs: Provider abstraction
# ─────────────────────────────────────────────────────────────────────────────
mkdir -p providers
cat > providers/mod.rs << 'PROVMODEOF'
//! LLM provider implementations (Ollama, OpenAI, Anthropic, Gemini).

pub mod ollama;
pub mod openai;
pub mod anthropic;
pub mod gemini;

pub use ollama::call_local_ollama;
pub use openai::call_openai;
pub use anthropic::call_anthropic;
pub use gemini::call_gemini;
PROVMODEOF

# ─────────────────────────────────────────────────────────────────────────────
# providers/ollama.rs: Lines 3890-3941
# ─────────────────────────────────────────────────────────────────────────────
cat > providers/ollama.rs << 'OLLAMAEOF'
//! Ollama (local) provider implementation.

use crate::config::Config;
use crate::types::*;
use axum::http::StatusCode;
use axum::Json;
use std::time::Duration;

OLLAMAEOF
sed -n '3890,3941p' main.rs >> providers/ollama.rs

# ─────────────────────────────────────────────────────────────────────────────
# providers/openai.rs: Lines 3943-3998
# ─────────────────────────────────────────────────────────────────────────────
cat > providers/openai.rs << 'OPENAIEOF'
//! OpenAI provider implementation.

use crate::config::{Config, ProviderConfig};
use crate::types::*;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use std::time::Duration;

OPENAIEOF
sed -n '3943,3998p' main.rs >> providers/openai.rs

# ─────────────────────────────────────────────────────────────────────────────
# providers/anthropic.rs: Lines 4000-4072
# ─────────────────────────────────────────────────────────────────────────────
cat > providers/anthropic.rs << 'ANTHROPICEOF'
//! Anthropic (Claude) provider implementation.

use crate::config::{Config, ProviderConfig};
use crate::types::*;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use std::time::Duration;

ANTHROPICEOF
sed -n '4000,4072p' main.rs >> providers/anthropic.rs

# ─────────────────────────────────────────────────────────────────────────────
# providers/gemini.rs: Lines 4074-4143
# ─────────────────────────────────────────────────────────────────────────────
cat > providers/gemini.rs << 'GEMINIEOF'
//! Google Gemini provider implementation.

use crate::config::{Config, ProviderConfig};
use crate::types::*;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use std::time::Duration;

GEMINIEOF
sed -n '4074,4143p' main.rs >> providers/gemini.rs

# ─────────────────────────────────────────────────────────────────────────────
# New main.rs: Just imports + main() entrypoint
# ─────────────────────────────────────────────────────────────────────────────
cat > main_new.rs << 'MAINEOF'
//! LLM Gateway - OpenAI-compatible API with local + premium routing.
//! 
//! Modules:
//! - config: Configuration loading
//! - types: API types and state
//! - auth: Authentication (API keys, JWT)
//! - fuel: Usage tracking and billing
//! - qc: Quality control sampling
//! - routing: Request routing and circuit breaker
//! - streaming: SSE streaming
//! - handlers: HTTP request handlers
//! - providers: LLM provider implementations

mod config;
mod types;
mod auth;
mod fuel;
mod qc;
mod routing;
mod streaming;
mod handlers;
mod providers;

use axum::{routing::{get, post}, Router};
use config::Config;
use handlers::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use types::{AppState, GatewayMetrics};

MAINEOF
# Main function
sed -n '4296,4632p' main.rs >> main_new.rs

echo "=== Files created ==="
ls -la *.rs providers/

echo ""
echo "=== Next steps ==="
echo "1. Review each file and add missing imports"
echo "2. Add 'pub' to functions that need to be exported"
echo "3. Run 'cargo check' to find issues"
echo "4. Fix cross-module references"
echo ""
echo "Backup saved as main.rs.backup"
