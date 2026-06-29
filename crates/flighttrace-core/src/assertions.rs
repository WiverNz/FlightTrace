//! Assertion outcomes recorded against a mission run.
//!
//! Issue 1 hard-codes assertion results inside the demo generator. Issue 5
//! will replace that with a real YAML-driven evaluator, but it will produce
//! the same [`AssertionResult`] shape.

use serde::{Deserialize, Serialize};

use crate::ids::{AssertionResultId, MissionRunId};
use crate::time::Timestamp;

/// Outcome of a single assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssertionOutcome {
    /// The assertion's expectation held.
    Pass,
    /// The assertion's expectation held with caveats worth surfacing.
    Warn,
    /// The assertion's expectation did not hold.
    Fail,
}

/// One assertion's verdict against a mission run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Identifier of this assertion result.
    pub id: AssertionResultId,
    /// Mission run this assertion was evaluated against.
    pub run_id: MissionRunId,
    /// Stable assertion name (e.g. `"takeoff_reaches_10m"`).
    pub name: String,
    /// Pass / warn / fail verdict.
    pub outcome: AssertionOutcome,
    /// Short detail string explaining the verdict.
    pub detail: String,
    /// When the assertion was evaluated.
    pub evaluated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn outcome_round_trips() {
        for outcome in [
            AssertionOutcome::Pass,
            AssertionOutcome::Warn,
            AssertionOutcome::Fail,
        ] {
            let json = serde_json::to_string(&outcome).unwrap();
            let back: AssertionOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(back, outcome);
        }
    }

    #[test]
    fn assertion_result_round_trips() {
        let result = AssertionResult {
            id: AssertionResultId::from_u128(0x01),
            run_id: MissionRunId::from_u128(0x02),
            name: "takeoff_reaches_10m".into(),
            outcome: AssertionOutcome::Fail,
            detail: "max_altitude=2.4m target=10.0m".into(),
            evaluated_at: Timestamp(datetime!(2025-01-01 00:00:12 UTC)),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: AssertionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back, result);
    }
}
