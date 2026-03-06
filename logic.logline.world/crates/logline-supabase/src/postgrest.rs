//! PostgREST query builder for arbitrary table access.
//!
//! Provides a fluent API for building PostgREST queries.

use crate::{Error, Result, SupabaseClient};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Query builder for PostgREST operations.
///
/// # CLI Usage
///
/// ```ignore
/// // Direct table query (used internally by CLI commands)
/// let users: Vec<User> = client
///     .from("users")
///     .select("*")
///     .eq("tenant_id", "tenant-123")
///     .limit(10)
///     .execute()
///     .await?;
/// ```
pub struct QueryBuilder<'a> {
    client: &'a SupabaseClient,
    table: String,
    select: Option<String>,
    filters: Vec<String>,
    order: Option<String>,
    limit: Option<u32>,
    single: bool,
}

impl SupabaseClient {
    /// Start a query on a table.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tenants: Vec<Tenant> = client
    ///     .from("tenants")
    ///     .select("tenant_id,slug,display_name")
    ///     .execute()
    ///     .await?;
    /// ```
    pub fn from(&self, table: &str) -> QueryBuilder<'_> {
        QueryBuilder {
            client: self,
            table: table.to_string(),
            select: None,
            filters: Vec::new(),
            order: None,
            limit: None,
            single: false,
        }
    }
}

impl<'a> QueryBuilder<'a> {
    /// Select specific columns.
    pub fn select(mut self, columns: &str) -> Self {
        self.select = Some(columns.to_string());
        self
    }

    /// Filter by equality.
    pub fn eq(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=eq.{}", column, value));
        self
    }

    /// Filter by inequality.
    pub fn neq(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=neq.{}", column, value));
        self
    }

    /// Filter by greater than.
    pub fn gt(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=gt.{}", column, value));
        self
    }

    /// Filter by greater than or equal.
    pub fn gte(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=gte.{}", column, value));
        self
    }

    /// Filter by less than.
    pub fn lt(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=lt.{}", column, value));
        self
    }

    /// Filter by less than or equal.
    pub fn lte(mut self, column: &str, value: &str) -> Self {
        self.filters.push(format!("{}=lte.{}", column, value));
        self
    }

    /// Filter by LIKE pattern.
    pub fn like(mut self, column: &str, pattern: &str) -> Self {
        self.filters.push(format!("{}=like.{}", column, pattern));
        self
    }

    /// Filter by value in list.
    pub fn in_list(mut self, column: &str, values: &[&str]) -> Self {
        let list = values.join(",");
        self.filters.push(format!("{}=in.({})", column, list));
        self
    }

    /// Order results.
    pub fn order(mut self, column: &str, ascending: bool) -> Self {
        let dir = if ascending { "asc" } else { "desc" };
        self.order = Some(format!("{}.{}", column, dir));
        self
    }

    /// Limit results.
    pub fn limit(mut self, n: u32) -> Self {
        self.limit = Some(n);
        self
    }

    /// Return only a single row (error if not exactly one).
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    /// Execute the query and deserialize results.
    pub async fn execute<T: DeserializeOwned>(self) -> Result<Vec<T>> {
        let mut url = format!("{}/{}", self.client.postgrest_url(), self.table);

        let mut params = Vec::new();
        if let Some(s) = self.select {
            params.push(format!("select={}", s));
        }
        params.extend(self.filters);
        if let Some(o) = self.order {
            params.push(format!("order={}", o));
        }
        if let Some(l) = self.limit {
            params.push(format!("limit={}", l));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let mut headers = self.client.auth_headers();
        if self.single {
            headers.insert(
                "Accept",
                "application/vnd.pgrst.object+json".parse().unwrap(),
            );
        }

        let response = self.client.http().get(&url).headers(headers).send().await?;

        if !response.status().is_success() {
            let error: Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        Ok(response.json().await?)
    }

    /// Insert a row and return it.
    pub async fn insert<T: Serialize + DeserializeOwned>(self, data: &T) -> Result<T> {
        let url = format!("{}/{}", self.client.postgrest_url(), self.table);

        let response = self
            .client
            .http()
            .post(&url)
            .headers(self.client.auth_headers())
            .json(data)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        let results: Vec<T> = response.json().await?;
        results.into_iter().next().ok_or_else(|| Error::PostgRest {
            code: "no_response".into(),
            message: "No row returned from insert".into(),
        })
    }

    /// Update rows matching filters.
    pub async fn update<T: Serialize>(self, data: &T) -> Result<u64> {
        let mut url = format!("{}/{}", self.client.postgrest_url(), self.table);

        if !self.filters.is_empty() {
            url.push('?');
            url.push_str(&self.filters.join("&"));
        }

        let mut headers = self.client.auth_headers();
        headers.insert("Prefer", "return=minimal,count=exact".parse().unwrap());

        let response = self
            .client
            .http()
            .patch(&url)
            .headers(headers)
            .json(data)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        // Parse count from content-range header
        let count = response
            .headers()
            .get("content-range")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split('/').last())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(count)
    }

    /// Delete rows matching filters.
    pub async fn delete(self) -> Result<u64> {
        let mut url = format!("{}/{}", self.client.postgrest_url(), self.table);

        if !self.filters.is_empty() {
            url.push('?');
            url.push_str(&self.filters.join("&"));
        }

        let mut headers = self.client.auth_headers();
        headers.insert("Prefer", "return=minimal,count=exact".parse().unwrap());

        let response = self
            .client
            .http()
            .delete(&url)
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            let error: Value = response.json().await?;
            return Err(Error::PostgRest {
                code: error["code"].as_str().unwrap_or("unknown").into(),
                message: error["message"].as_str().unwrap_or("unknown error").into(),
            });
        }

        let count = response
            .headers()
            .get("content-range")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split('/').last())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Ok(count)
    }
}
