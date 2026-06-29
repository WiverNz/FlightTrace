//! FlightTrace core domain model.
//!
//! `flighttrace-core` is the single source of truth for FlightTrace's domain
//! types. Every other crate in the workspace depends on the types defined here
//! and never redefines them. The contract that flows through the system —
//! demo generator, storage, API, UI, assertion engine — is the set of
//! normalized events and telemetry samples produced for a mission run.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod assertions;
pub mod error;
pub mod events;
pub mod failure;
pub mod ids;
pub mod mission;
pub mod telemetry;
pub mod time;

pub use assertions::{AssertionOutcome, AssertionResult};
pub use error::FlightTraceError;
pub use events::{NormalizedEvent, NormalizedEventKind};
pub use failure::{Evidence, FailureReason, FailureReport};
pub use ids::{AssertionResultId, EventId, FailureReportId, MissionRunId};
pub use mission::{MissionPhase, MissionRun, MissionSource, MissionStatus, PhaseTransition};
pub use telemetry::TelemetrySample;
pub use time::Timestamp;
