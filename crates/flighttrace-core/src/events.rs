//! Normalized events emitted by sources (demo generator, MAVLink ingestion).
//!
//! Normalized events are the contract every other layer consumes. Sources
//! (issue 4's MAVLink ingestion, issue 1's deterministic demo) are
//! interchangeable; downstream code only sees [`NormalizedEvent`]s and
//! [`crate::TelemetrySample`]s.

use serde::{Deserialize, Serialize};

use crate::failure::FailureReason;
use crate::ids::{EventId, MissionRunId};
use crate::time::Timestamp;

/// One observed, vehicle-agnostic event tied to a mission run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedEvent {
    /// Identifier of this event.
    pub id: EventId,
    /// Mission run the event belongs to.
    pub run_id: MissionRunId,
    /// When the event happened.
    pub at: Timestamp,
    /// What happened.
    pub kind: NormalizedEventKind,
}

/// Discriminated union of all normalized event kinds.
///
/// Serialized using an internal `"kind"` tag so each event reads naturally as
/// JSON, e.g. `{"kind":"ArmedChanged","armed":true}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum NormalizedEventKind {
    /// First contact with the vehicle was established.
    VehicleConnected,
    /// A MAVLink `HEARTBEAT` (or equivalent) was received.
    HeartbeatReceived,
    /// The vehicle armed or disarmed.
    ArmedChanged {
        /// `true` if the vehicle is now armed, `false` if disarmed.
        armed: bool,
    },
    /// The flight mode changed.
    ModeChanged {
        /// New flight mode (autopilot-specific string, e.g. `"GUIDED"`).
        mode: String,
    },
    /// The vehicle acknowledged a command with success.
    CommandAccepted {
        /// MAVLink command name (e.g. `"MAV_CMD_NAV_TAKEOFF"`).
        command: String,
    },
    /// The vehicle rejected a command.
    CommandRejected {
        /// MAVLink command name.
        command: String,
        /// Human-readable rejection reason.
        reason: String,
    },
    /// The vehicle reported a new global position.
    PositionUpdated {
        /// Latitude in degrees.
        lat: f64,
        /// Longitude in degrees.
        lon: f64,
    },
    /// The vehicle reported a new altitude (relative or AMSL, source-defined).
    AltitudeUpdated {
        /// Altitude in metres.
        altitude_m: f64,
    },
    /// Battery state changed.
    BatteryUpdated {
        /// Estimated remaining charge as a percentage `0..=100`.
        remaining_pct: f32,
        /// Pack voltage in volts.
        voltage_v: f32,
    },
    /// The active mission waypoint changed.
    WaypointChanged {
        /// Zero-based waypoint index.
        index: u32,
    },
    /// Telemetry stopped flowing for longer than the configured threshold.
    TelemetryGapDetected {
        /// Duration of the gap in seconds.
        gap_sec: f64,
    },
    /// A `STATUSTEXT` line from the vehicle.
    StatustextReceived {
        /// MAVLink severity `0..=7` (lower = more severe).
        severity: u8,
        /// The statustext message text.
        text: String,
    },
    /// An assertion was evaluated and passed.
    AssertionPassed {
        /// Name of the assertion.
        assertion_name: String,
    },
    /// An assertion was evaluated and failed.
    AssertionFailed {
        /// Name of the assertion.
        assertion_name: String,
        /// Short detail explaining the failure.
        detail: String,
    },
    /// The mission failed with the given reason.
    MissionFailed {
        /// Canonical failure reason.
        reason: FailureReason,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn ts() -> Timestamp {
        Timestamp(datetime!(2025-01-01 00:00:00 UTC))
    }

    fn all_kinds() -> Vec<NormalizedEventKind> {
        vec![
            NormalizedEventKind::VehicleConnected,
            NormalizedEventKind::HeartbeatReceived,
            NormalizedEventKind::ArmedChanged { armed: true },
            NormalizedEventKind::ModeChanged {
                mode: "GUIDED".into(),
            },
            NormalizedEventKind::CommandAccepted {
                command: "MAV_CMD_NAV_TAKEOFF".into(),
            },
            NormalizedEventKind::CommandRejected {
                command: "MAV_CMD_DO_SET_MODE".into(),
                reason: "denied".into(),
            },
            NormalizedEventKind::PositionUpdated {
                lat: 47.3977,
                lon: 8.5456,
            },
            NormalizedEventKind::AltitudeUpdated { altitude_m: 2.4 },
            NormalizedEventKind::BatteryUpdated {
                remaining_pct: 85.0,
                voltage_v: 12.4,
            },
            NormalizedEventKind::WaypointChanged { index: 1 },
            NormalizedEventKind::TelemetryGapDetected { gap_sec: 3.2 },
            NormalizedEventKind::StatustextReceived {
                severity: 4,
                text: "EKF altitude variance".into(),
            },
            NormalizedEventKind::AssertionPassed {
                assertion_name: "vehicle_connected".into(),
            },
            NormalizedEventKind::AssertionFailed {
                assertion_name: "takeoff_reaches_10m".into(),
                detail: "max_altitude=2.4m target=10.0m".into(),
            },
            NormalizedEventKind::MissionFailed {
                reason: FailureReason::TakeoffTimeout,
            },
        ]
    }

    #[test]
    fn every_normalized_event_kind_round_trips() {
        for kind in all_kinds() {
            let json = serde_json::to_string(&kind).expect("serialize kind");
            assert!(
                json.contains("\"kind\""),
                "expected internally-tagged kind: {json}"
            );
            let back: NormalizedEventKind = serde_json::from_str(&json).expect("deserialize kind");
            assert_eq!(back, kind, "round-trip mismatch for {kind:?}");
        }
    }

    #[test]
    fn normalized_event_wrapper_round_trips() {
        let event = NormalizedEvent {
            id: EventId::from_u128(0x01),
            run_id: MissionRunId::from_u128(0x02),
            at: ts(),
            kind: NormalizedEventKind::MissionFailed {
                reason: FailureReason::TakeoffTimeout,
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: NormalizedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, event);
    }
}
