//! Per-check evaluators.
//!
//! Each module here owns one v0.1 check. A check is a pure function that
//! takes a step name, the per-check threshold, an evaluation window, and the
//! run's events + samples, and returns a [`CheckOutput`] with the verdict,
//! the human-readable detail, the evidence the verdict was based on, and the
//! moment the engine considered the check to be decided.

use flighttrace_core::{AssertionOutcome, Evidence, Timestamp};

pub mod altitude;
pub mod armed;
pub mod gap;
pub mod heartbeat;
pub mod mode;

/// Verdict + supporting context returned by one check.
#[derive(Debug, Clone)]
pub struct CheckOutput {
    /// Whether the check passed, warned, or failed.
    pub outcome: AssertionOutcome,
    /// Human-readable detail string surfaced in the API/UI.
    pub detail: String,
    /// Evidence items the check considered.
    pub evidence: Vec<Evidence>,
    /// When the check was decided (used as `AssertionResult.evaluated_at`).
    pub evaluated_at: Timestamp,
}

/// Inclusive time window a check evaluates over.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    /// Lower bound (typically the run's `started_at`).
    pub start: Timestamp,
    /// Upper bound (start + step `within_sec`, or the run's end).
    pub end: Timestamp,
}

impl Window {
    /// Returns true if `at` falls within `[start, end]`.
    pub fn contains(&self, at: Timestamp) -> bool {
        at.0 >= self.start.0 && at.0 <= self.end.0
    }
}
