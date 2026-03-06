//! Fuel event emission and querying.
//!
//! Fuel events are the canonical billing ledger for the LogLine ecosystem.
//! All billable actions emit fuel events, which are append-only and immutable.

use crate::{Error, Result, SupabaseClient};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A fuel event representing billable resource consumption.
///
/// # CLI Usage
///
/// ```ignore
/// // From CLI: logline fuel emit --app-id llm-gateway --units 1000 --unit-type llm_tokens
/// let event = FuelEvent {
///     idempotency_key: "llm-gateway:req-123:2026-03-02T10:00".into(),
///     tenant_id: "tenant-123".into(),
///     app_id: "llm-gateway".into(),
///     user_id: "user-456".into(),
///     units: 1000.0,
///     unit_type: "llm_tokens".into(),
///     occurred_at: Utc::now(),
///     source: "anthropic:claude-3-opus".into(),
///     metadata: Some(json!({"model": "claude-3-opus"})),
/// };
/// client.emit_fuel(event).await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelEvent {
    /// Client-provided idempotency key to prevent duplicates.
    /// Pattern: `{app_id}:{action_type}:{unique_id}:{timestamp_window}`
    pub idempotency_key: String,

    /// Tenant (workspace/organization) being billed.
    pub tenant_id: String,

    /// Application that generated the usage.
    pub app_id: String,

    /// User who triggered the action.
    pub user_id: String,

    /// Quantity consumed.
    pub units: f64,

    /// Unit classification (e.g., `llm_tokens`, `code_job`, `storage_bytes`).
    pub unit_type: String,

    /// When the usage occurred.
    pub occurred_at: DateTime<Utc>,

    /// Origin subsystem (e.g., `anthropic:claude-3-opus`, `code247:pipeline`).
    pub source: String,

    /// Additional context (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Filter for querying fuel events.
#[derive(Debug, Default, Clone)]
pub struct FuelFilter {
    pub tenant_id: Option<String>,
    pub app_id: Option<String>,
    pub user_id: Option<String>,
    pub unit_type: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

impl FuelFilter {
    /// Filter by tenant.
    pub fn for_tenant(tenant_id: impl Into<String>) -> Self {
        Self {
            tenant_id: Some(tenant_id.into()),
            ..Default::default()
        }
    }

    /// Filter by user.
    pub fn for_user(user_id: impl Into<String>) -> Self {
        Self {
            user_id: Some(user_id.into()),
            ..Default::default()
        }
    }

    /// Add app filter.
    pub fn app(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    /// Add time range filter.
    pub fn time_range(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        self.from = Some(from);
        self.to = Some(to);
        self
    }

    /// Limit results.
    pub fn limit(mut self, n: u32) -> Self {
        self.limit = Some(n);
        self
    }
}

/// Stored fuel event (includes server-generated fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredFuelEvent {
    pub event_id: String,
    #[serde(flatten)]
    pub event: FuelEvent,
    pub created_at: DateTime<Utc>,
}

impl SupabaseClient {
    /// Emit a fuel event to the ledger.
    ///
    /// This is an append-only operation. The event will be rejected if the
    /// `idempotency_key` already exists (returning `Error::DuplicateEvent`).
    ///
    /// # CLI Command
    ///
    /// ```bash
    /// logline fuel emit \
    ///   --app-id llm-gateway \
    ///   --units 1000 \
    ///   --unit-type llm_tokens \
    ///   --source "anthropic:claude-3-opus" \
    ///   --idempotency-key "manual:2026-03-02:001"
    /// ```
    pub async fn emit_fuel(&self, event: FuelEvent) -> Result<StoredFuelEvent> {
        validate_fuel_metadata(event.metadata.as_ref(), &event.unit_type)?;

        let url = format!("{}/fuel_events", self.postgrest_url());

        let response = self
            .http()
            .post(&url)
            .headers(self.auth_headers())
            .json(&event)
            .send()
            .await?;

        if response.status() == 409 {
            return Err(Error::DuplicateEvent);
        }

        if !response.status().is_success() {
            let error: serde_json::Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        let events: Vec<StoredFuelEvent> = response.json().await?;
        events.into_iter().next().ok_or_else(|| Error::PostgRest {
            code: "no_response".into(),
            message: "No event returned from insert".into(),
        })
    }

    /// Query fuel events with filters.
    ///
    /// # CLI Command
    ///
    /// ```bash
    /// logline fuel list --tenant tenant-123 --app llm-gateway --limit 50
    /// ```
    pub async fn query_fuel(&self, filter: FuelFilter) -> Result<Vec<StoredFuelEvent>> {
        let mut url = format!(
            "{}/fuel_events?order=occurred_at.desc",
            self.postgrest_url()
        );

        if let Some(ref tenant_id) = filter.tenant_id {
            url.push_str(&format!("&tenant_id=eq.{}", tenant_id));
        }
        if let Some(ref app_id) = filter.app_id {
            url.push_str(&format!("&app_id=eq.{}", app_id));
        }
        if let Some(ref user_id) = filter.user_id {
            url.push_str(&format!("&user_id=eq.{}", user_id));
        }
        if let Some(ref unit_type) = filter.unit_type {
            url.push_str(&format!("&unit_type=eq.{}", unit_type));
        }
        if let Some(from) = filter.from {
            url.push_str(&format!("&occurred_at=gte.{}", from.to_rfc3339()));
        }
        if let Some(to) = filter.to {
            url.push_str(&format!("&occurred_at=lte.{}", to.to_rfc3339()));
        }
        if let Some(limit) = filter.limit {
            url.push_str(&format!("&limit={}", limit));
        }

        let response = self
            .http()
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let error: serde_json::Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        Ok(response.json().await?)
    }

    /// Get fuel summary for a tenant (total units by app and unit_type).
    ///
    /// # CLI Command
    ///
    /// ```bash
    /// logline fuel summary --tenant tenant-123 --month 2026-03
    /// ```
    pub async fn fuel_summary(
        &self,
        tenant_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<FuelSummary>> {
        // Use RPC for aggregation (requires a Postgres function)
        // Fallback: fetch and aggregate client-side
        let events = self
            .query_fuel(
                FuelFilter::for_tenant(tenant_id)
                    .time_range(from, to)
                    .limit(10000),
            )
            .await?;

        // Client-side aggregation
        use std::collections::HashMap;
        let mut map: HashMap<(String, String), f64> = HashMap::new();

        for event in events {
            let key = (event.event.app_id.clone(), event.event.unit_type.clone());
            *map.entry(key).or_default() += event.event.units;
        }

        Ok(map
            .into_iter()
            .map(|((app_id, unit_type), total_units)| FuelSummary {
                app_id,
                unit_type,
                total_units,
            })
            .collect())
    }
}

fn validate_fuel_metadata(
    metadata: Option<&serde_json::Value>,
    unit_type: &str,
) -> Result<()> {
    let Some(metadata) = metadata else {
        return Err(Error::Validation("metadata is required".into()));
    };

    let Some(map) = metadata.as_object() else {
        return Err(Error::Validation("metadata must be a JSON object".into()));
    };

    for key in ["event_type", "trace_id", "outcome"] {
        let Some(value) = map.get(key).and_then(serde_json::Value::as_str) else {
            return Err(Error::Validation(format!(
                "missing or invalid metadata.{key}"
            )));
        };
        if value.trim().is_empty() {
            return Err(Error::Validation(format!(
                "metadata.{key} cannot be empty"
            )));
        }
    }

    if !map.contains_key("parent_event_id") {
        return Err(Error::Validation("missing metadata.parent_event_id".into()));
    }
    if !map
        .get("parent_event_id")
        .map(|value| value.is_null() || value.is_string())
        .unwrap_or(false)
    {
        return Err(Error::Validation(
            "metadata.parent_event_id must be string|null".into(),
        ));
    }

    if unit_type == "llm_tokens" {
        for key in ["provider", "model"] {
            let Some(value) = map.get(key).and_then(serde_json::Value::as_str) else {
                return Err(Error::Validation(format!(
                    "missing or invalid metadata.{key}"
                )));
            };
            if value.trim().is_empty() {
                return Err(Error::Validation(format!(
                    "metadata.{key} cannot be empty"
                )));
            }
        }

        for key in ["prompt_tokens", "completion_tokens", "latency_ms"] {
            if !map
                .get(key)
                .map(serde_json::Value::is_number)
                .unwrap_or(false)
            {
                return Err(Error::Validation(format!(
                    "missing or invalid metadata.{key}"
                )));
            }
        }
    }

    Ok(())
}

/// Aggregated fuel summary by app and unit type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelSummary {
    pub app_id: String,
    pub unit_type: String,
    pub total_units: f64,
}
