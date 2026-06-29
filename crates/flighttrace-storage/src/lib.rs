//! SQLite-backed persistence for FlightTrace.
//!
//! The storage layer takes [`flighttrace_core`] domain values and persists
//! them via `sqlx`. Each domain aggregate has a small repository struct
//! (`MissionRunRepo`, `NormalizedEventRepo`, …) that owns the SQL for its
//! table; a higher-level [`bundle`] module persists or loads a complete run
//! (`RunBundle`) in a single transaction.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod assertions;
pub mod bundle;
mod encoding;
pub mod error;
pub mod events;
pub mod failure;
pub mod pool;
pub mod runs;
pub mod telemetry;

pub use assertions::AssertionResultRepo;
pub use bundle::{RunBundle, load_bundle, persist_bundle};
pub use error::{StorageError, StorageResult};
pub use events::NormalizedEventRepo;
pub use failure::FailureReportRepo;
pub use pool::{MIGRATOR, connect, connect_in_memory};
pub use runs::MissionRunRepo;
pub use telemetry::TelemetrySampleRepo;
