//! Core error type shared across FlightTrace crates.

use thiserror::Error;

/// Errors produced by FlightTrace core operations.
///
/// `flighttrace-core` itself is mostly pure data types; this enum exists so
/// downstream crates (storage, API, assertions) can map their own errors
/// onto a small, shared vocabulary at the boundary.
#[derive(Debug, Error)]
pub enum FlightTraceError {
    /// Input could not be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// Input parsed but failed validation.
    #[error("validation error: {0}")]
    Validation(String),

    /// A referenced entity was not found.
    #[error("not found: {0}")]
    NotFound(String),
}
