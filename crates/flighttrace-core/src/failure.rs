//! Failure reasons, evidence items, and failure reports.

use serde::{Deserialize, Serialize};

use crate::ids::{EventId, FailureReportId, MissionRunId};
use crate::time::Timestamp;

/// Canonical machine-readable reason a mission run failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureReason {
    /// No vehicle heartbeat was received within the expected window.
    NoHeartbeat,
    /// A MAVLink command was sent but never acknowledged.
    CommandTimeout,
    /// The vehicle did not reach the target altitude within the time budget.
    TakeoffTimeout,
    /// A waypoint was not reached.
    WaypointNotReached,
    /// The flight mode changed in a way the mission did not allow.
    UnexpectedModeChange,
    /// Telemetry stopped flowing for longer than the configured gap.
    TelemetryGap,
    /// The battery dropped below the safe threshold.
    LowBattery,
    /// The GPS solution degraded below the required quality.
    GpsUnhealthy,
    /// Catch-all for failures that could not be attributed to a specific
    /// reason. Should be rare.
    Unknown,
}

/// A single piece of evidence cited in a [`FailureReport`].
///
/// Evidence items intentionally point at facts that exist elsewhere in the
/// run (events, telemetry samples, statustexts) rather than reformatting them.
/// Free-form notes are allowed for the small amount of human-written context
/// every failure report needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Evidence {
    /// A specific normalized event from the run.
    Event(EventId),
    /// A scalar field on a telemetry sample at a specific time.
    Sample {
        /// When the sample was recorded.
        at: Timestamp,
        /// Sample field name, e.g. `"altitude_m"`.
        field: String,
        /// Stringified value at that field.
        value: String,
    },
    /// A vehicle-emitted `STATUSTEXT` line.
    Statustext {
        /// When the statustext was observed.
        at: Timestamp,
        /// The statustext message text.
        text: String,
    },
    /// A short human-written note expanding on the report.
    Note(String),
}

/// Structured explanation of why a mission run failed.
///
/// A [`FailureReport`] is the headline artefact of FlightTrace: when a mission
/// run ends in [`crate::MissionStatus::Failed`], this is what answers "why?".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureReport {
    /// Identifier of this failure report.
    pub id: FailureReportId,
    /// Mission run this report explains.
    pub run_id: MissionRunId,
    /// Canonical reason the run failed.
    pub reason: FailureReason,
    /// One-line human summary (e.g. `"Mission failed: TAKEOFF_TIMEOUT"`).
    pub summary: String,
    /// Ordered list of evidence items supporting the failure conclusion.
    pub evidence: Vec<Evidence>,
    /// Short paragraph describing the likely cause and where to look next.
    pub likely_cause: String,
    /// When the report was produced.
    pub generated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn ts() -> Timestamp {
        Timestamp(datetime!(2025-01-01 00:00:00 UTC))
    }

    #[test]
    fn failure_reason_round_trips() {
        for reason in [
            FailureReason::NoHeartbeat,
            FailureReason::CommandTimeout,
            FailureReason::TakeoffTimeout,
            FailureReason::WaypointNotReached,
            FailureReason::UnexpectedModeChange,
            FailureReason::TelemetryGap,
            FailureReason::LowBattery,
            FailureReason::GpsUnhealthy,
            FailureReason::Unknown,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let back: FailureReason = serde_json::from_str(&json).unwrap();
            assert_eq!(back, reason);
        }
    }

    #[test]
    fn evidence_variants_round_trip() {
        let cases = vec![
            Evidence::Event(EventId::from_u128(0xAA)),
            Evidence::Sample {
                at: ts(),
                field: "altitude_m".into(),
                value: "2.4".into(),
            },
            Evidence::Statustext {
                at: ts(),
                text: "EKF altitude variance".into(),
            },
            Evidence::Note("vehicle stopped climbing".into()),
        ];
        for ev in cases {
            let json = serde_json::to_string(&ev).unwrap();
            let back: Evidence = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ev);
        }
    }

    #[test]
    fn failure_report_round_trips() {
        let report = FailureReport {
            id: FailureReportId::from_u128(0x01),
            run_id: MissionRunId::from_u128(0x02),
            reason: FailureReason::TakeoffTimeout,
            summary: "Mission failed: TAKEOFF_TIMEOUT".into(),
            evidence: vec![Evidence::Note("hello".into())],
            likely_cause: "Vehicle stopped climbing.".into(),
            generated_at: ts(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let back: FailureReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, report);
    }
}
