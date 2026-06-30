//! YAML-driven assertion engine for FlightTrace mission runs.
//!
//! Public entry points:
//!
//! - [`parse_mission_yaml`] / [`load_mission_file`] — turn a YAML document or
//!   a file on disk into a [`MissionSpec`].
//! - [`evaluate`] — run a [`MissionSpec`] against a run's events and samples
//!   and return one [`flighttrace_core::AssertionResult`] per declared check
//!   plus an optional [`flighttrace_core::FailureReport`].
//!
//! The engine is *pure*: same inputs in, same outputs out. There is no I/O,
//! no clock dependency, and no shared state. Callers persist the results
//! themselves (see `flighttrace-storage` and the `/api/runs/:id/reevaluate`
//! handler).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod checks;
pub mod engine;
pub mod error;
pub mod failure;
pub mod parser;
pub mod schema;

pub use engine::{EvaluationInput, EvaluationOutput, evaluate};
pub use error::{AssertionError, AssertionResult};
pub use failure::{
    CheckKind, assertion_id_for, likely_cause_for, reason_for_failure, report_id_for,
};
pub use parser::{load_mission_file, parse_mission_yaml};
pub use schema::{ExpectSpec, MissionSpec, StepSpec};
