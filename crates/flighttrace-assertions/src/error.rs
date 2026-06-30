//! Error type for parsing and loading mission specs.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Errors returned while loading or parsing a mission YAML.
#[derive(Debug, Error)]
pub enum AssertionError {
    /// Underlying YAML deserialization failure.
    #[error("invalid mission YAML: {0}")]
    Parse(#[from] serde_yaml::Error),

    /// Failure to read the mission file from disk.
    #[error("read mission file {path}: {source}")]
    Io {
        /// The file we tried to open.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// A semantic problem the schema can't express (e.g. empty mission name).
    #[error("invalid mission spec: {0}")]
    Invalid(String),
}

/// Convenience alias for results from this crate.
pub type AssertionResult<T> = std::result::Result<T, AssertionError>;
