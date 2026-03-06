//! Unified Supabase client for LogLine ecosystem services.
//!
//! Provides:
//! - JWT validation via Supabase JWKS
//! - Fuel event emission to `fuel_events` table
//! - Object storage operations
//! - Realtime broadcast
//! - PostgREST query builder

mod client;
mod error;
mod fuel;
mod postgrest;
mod realtime;
mod storage;

pub use client::{SupabaseClient, SupabaseConfig};
pub use error::{Error, Result};
pub use fuel::{FuelEvent, FuelFilter};
pub use postgrest::QueryBuilder;
