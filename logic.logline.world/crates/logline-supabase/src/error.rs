//! Error types for Supabase client operations.

use thiserror::Error;

/// Result type alias for Supabase operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during Supabase operations.
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// JWT validation failed.
    #[error("JWT validation error: {0}")]
    Jwt(String),

    /// PostgREST returned an error.
    #[error("PostgREST error: {message} (code: {code})")]
    PostgRest { code: String, message: String },

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Realtime broadcast failed.
    #[error("Realtime error: {0}")]
    Realtime(String),

    /// Configuration error.
    #[error("Config error: {0}")]
    Config(String),

    /// Validation error on client-provided payload.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Idempotency key conflict (duplicate event).
    #[error("Duplicate event: idempotency_key already exists")]
    DuplicateEvent,
}
