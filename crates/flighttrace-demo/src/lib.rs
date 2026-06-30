//! Deterministic demo mission runs for FlightTrace.
//!
//! Issue 1 ships a single scenario — a takeoff that never reaches its target
//! altitude and times out with `TAKEOFF_TIMEOUT`. The bundle returned by
//! [`generate_failed_takeoff_run`] is byte-for-byte identical across calls,
//! which makes it usable as a fixture for downstream storage, API, and UI
//! tests.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

use flighttrace_core::{
    AssertionResult, FailureReport, MissionRun, NormalizedEvent, TelemetrySample,
};

pub mod takeoff_timeout;

/// A complete in-memory mission run with all its child collections.
///
/// Issue 2 will persist this bundle into SQLite; for now it is the unit of
/// exchange between the demo generator and the CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemoRun {
    /// Headline metadata for the run.
    pub run: MissionRun,
    /// Normalized events that happened during the run, in chronological order.
    pub events: Vec<NormalizedEvent>,
    /// Telemetry samples for the run, in chronological order.
    pub telemetry: Vec<TelemetrySample>,
    /// Assertion verdicts for the run.
    pub assertions: Vec<AssertionResult>,
    /// Failure report explaining why the run failed.
    pub failure_report: FailureReport,
}

/// Generate the deterministic `TAKEOFF_TIMEOUT` demo run.
///
/// Two calls to this function return equal values (and therefore identical
/// JSON output). All identifiers, timestamps, and numeric values are fixed.
pub fn generate_failed_takeoff_run() -> DemoRun {
    takeoff_timeout::build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flighttrace_core::{
        AssertionOutcome, Evidence, FailureReason, MissionStatus, NormalizedEventKind,
    };

    #[test]
    fn generator_is_deterministic() {
        let a = generate_failed_takeoff_run();
        let b = generate_failed_takeoff_run();
        assert_eq!(a, b, "in-memory bundles must be equal across calls");

        let a_json = serde_json::to_string_pretty(&a).unwrap();
        let b_json = serde_json::to_string_pretty(&b).unwrap();
        assert_eq!(a_json, b_json, "JSON output must be byte-identical");
    }

    #[test]
    fn run_is_failed_takeoff_timeout() {
        let bundle = generate_failed_takeoff_run();
        assert_eq!(bundle.run.status, MissionStatus::Failed);
        assert_eq!(
            bundle.run.failure_reason,
            Some(FailureReason::TakeoffTimeout)
        );
    }

    #[test]
    fn timeline_includes_required_phase_transitions() {
        use flighttrace_core::MissionPhase::*;
        let bundle = generate_failed_takeoff_run();
        let phases: Vec<_> = bundle.run.phases.iter().map(|p| p.phase).collect();
        for required in [Connected, Armed, TakeoffRequested, Climbing, Failed] {
            assert!(
                phases.contains(&required),
                "mission timeline must include {required:?}; got {phases:?}"
            );
        }
    }

    #[test]
    fn events_include_mission_failed_and_ekf_statustext() {
        let bundle = generate_failed_takeoff_run();
        let mut saw_mission_failed = false;
        let mut saw_ekf_statustext = false;
        for event in &bundle.events {
            match &event.kind {
                NormalizedEventKind::MissionFailed {
                    reason: FailureReason::TakeoffTimeout,
                } => saw_mission_failed = true,
                NormalizedEventKind::StatustextReceived { text, .. } if text.contains("EKF") => {
                    saw_ekf_statustext = true
                }
                _ => {}
            }
        }
        assert!(saw_mission_failed, "expected a MissionFailed event");
        assert!(
            saw_ekf_statustext,
            "expected a StatustextReceived with text containing 'EKF'"
        );
    }

    #[test]
    fn max_altitude_is_between_2_and_3_metres() {
        let bundle = generate_failed_takeoff_run();
        let max = bundle
            .telemetry
            .iter()
            .map(|s| s.altitude_m)
            .fold(f64::MIN, f64::max);
        assert!(
            (2.0..=3.0).contains(&max),
            "max altitude {max} not in [2.0, 3.0]"
        );
    }

    #[test]
    fn telemetry_starts_at_zero_and_stalls_near_target() {
        let bundle = generate_failed_takeoff_run();
        let first = bundle.telemetry.first().expect("telemetry not empty");
        assert!(
            first.altitude_m.abs() < 1e-9,
            "telemetry should start at 0m"
        );

        // The last few samples should all sit at the stall altitude.
        let tail = &bundle.telemetry[bundle.telemetry.len() - 3..];
        let stall = tail[0].altitude_m;
        for sample in tail {
            assert!(
                (sample.altitude_m - stall).abs() < 1e-9,
                "trailing telemetry should stall at the same altitude"
            );
        }
    }

    #[test]
    fn has_passing_and_failing_assertions() {
        let bundle = generate_failed_takeoff_run();
        let pass_count = bundle
            .assertions
            .iter()
            .filter(|a| a.outcome == AssertionOutcome::Pass)
            .count();
        let fail = bundle
            .assertions
            .iter()
            .find(|a| a.outcome == AssertionOutcome::Fail);
        assert!(pass_count >= 1, "expected at least one passing assertion");
        let fail = fail.expect("expected at least one failing assertion");
        assert_eq!(fail.name, "takeoff.altitude_at_least_m");
    }

    #[test]
    fn failure_report_reason_matches_run() {
        let bundle = generate_failed_takeoff_run();
        assert_eq!(
            Some(bundle.failure_report.reason),
            bundle.run.failure_reason
        );
        assert_eq!(bundle.failure_report.run_id, bundle.run.id);
    }

    #[test]
    fn failure_report_has_evidence_with_statustext() {
        let bundle = generate_failed_takeoff_run();
        assert!(
            !bundle.failure_report.evidence.is_empty(),
            "failure report evidence must be non-empty"
        );
        let has_statustext = bundle
            .failure_report
            .evidence
            .iter()
            .any(|e| matches!(e, Evidence::Statustext { .. }));
        assert!(
            has_statustext,
            "failure report should cite at least one statustext"
        );
    }

    #[test]
    fn failure_report_summary_and_cause_present() {
        let bundle = generate_failed_takeoff_run();
        assert_eq!(
            bundle.failure_report.summary,
            "Mission failed: TAKEOFF_TIMEOUT"
        );
        assert!(
            bundle
                .failure_report
                .likely_cause
                .to_lowercase()
                .contains("altitude"),
            "likely cause should mention altitude"
        );
    }

    #[test]
    fn bundle_round_trips_through_json() {
        let bundle = generate_failed_takeoff_run();
        let json = serde_json::to_string(&bundle).unwrap();
        let back: DemoRun = serde_json::from_str(&json).unwrap();
        assert_eq!(back, bundle);
    }
}
