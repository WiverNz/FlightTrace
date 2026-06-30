//! Error type for MAVLink ingestion.

use thiserror::Error;

/// Errors that can surface from MAVLink ingestion.
///
/// Most errors are non-fatal at the loop level (the listener logs and keeps
/// reading); fatal-by-default errors are bind / config failures and storage
/// errors that have already cost data.
#[derive(Debug, Error)]
pub enum MavlinkIngestError {
    /// The user-supplied endpoint string was not a `udp://host:port` URL.
    #[error("invalid MAVLink endpoint `{0}`: {1}")]
    InvalidEndpoint(String, String),

    /// `mavlink::connect_async` failed (usually bind / address errors).
    #[error("MAVLink connect failed: {0}")]
    Connect(#[source] std::io::Error),

    /// A storage call failed while persisting a run / event / sample.
    #[error("storage error: {0}")]
    Storage(#[from] flighttrace_storage::StorageError),
}

/// Convenience alias.
pub type MavlinkIngestResult<T> = Result<T, MavlinkIngestError>;
