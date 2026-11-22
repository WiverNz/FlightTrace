// FlightTrace MAVLink ingestion is strictly read-only. Do not add send paths.
//
// This crate only ever calls `recv` on a MAVLink connection. Any code that
// reaches for `send`, `send_default`, `send_frame`, command upload, mode
// change, parameter write, or any other "talk to the vehicle" API must be
// rejected at review. The read-only guarantee is the contract issue 4 was
// scoped around (see docs/issues/04-mavlink-ingestion.md).

//! Read-only MAVLink UDP ingestion for FlightTrace.
//!
//! Translates a stream of MAVLink messages from PX4/ArduPilot SITL (or a real
//! vehicle) into the same [`flighttrace_core`] normalized event +
//! [`flighttrace_core::TelemetrySample`] shape the demo and assertion engine
//! consume. The listener never writes to the socket; FlightTrace is an
//! observability tool, not a ground station.
//!
//! Entry point: [`listener::run_ingestion`]. The CLI wires this up in
//! `flighttrace-cli` behind the optional `--mavlink` flag.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod error;
pub mod gap;
pub mod listener;
pub mod session;
pub mod tlog;
pub mod translate;

pub use config::{Dialect, MavlinkConfig, parse_endpoint};
pub use error::{MavlinkIngestError, MavlinkIngestResult};
pub use listener::run_ingestion;
pub use tlog::import_tlog;
