//! Storage error type.

use thiserror::Error;

/// Errors returned by the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The requested entity was not found.
    #[error("not found")]
    NotFound,

    /// A database error from sqlx.
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    /// A migration failed to apply.
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),

    /// A JSON payload could not be (de)serialized.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

/// Convenience alias used throughout the storage layer.
pub type StorageResult<T> = Result<T, StorageError>;
