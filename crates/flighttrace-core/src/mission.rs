//! Mission run, phases, and status.

use serde::{Deserialize, Serialize};

use crate::failure::FailureReason;
use crate::ids::MissionRunId;
use crate::time::Timestamp;

/// Ordered phases a mission run can move through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionPhase {
    /// No contact with the vehicle yet.
    Disconnected,
    /// Vehicle is reachable; heartbeat received.
    Connected,
    /// Arm sequence is in progress.
    Arming,
    /// Vehicle is armed and idle on the ground.
    Armed,
    /// A takeoff command has been accepted; vehicle is preparing to climb.
    TakeoffRequested,
    /// Vehicle is actively climbing toward the target altitude.
    Climbing,
    /// Vehicle has reached the target takeoff altitude.
    TargetAltitudeReached,
    /// Vehicle is in transit to the next mission waypoint.
    NavigatingToWaypoint,
    /// Vehicle is returning to home.
    ReturningHome,
    /// Vehicle is performing a landing approach.
    Landing,
    /// Vehicle is on the ground and disarmed.
    Landed,
    /// Mission ended in failure.
    Failed,
}

/// High-level status of a mission run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionStatus {
    /// The run is in progress.
    Running,
    /// The run completed successfully.
    Succeeded,
    /// The run ended in failure (see [`FailureReason`]).
    Failed,
}

/// Origin of a mission run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionSource {
    /// Generated deterministically by [`flighttrace-demo`](../flighttrace_demo/index.html).
    Demo,
    /// Ingested live from a MAVLink source (issue 4).
    Mavlink,
}

/// A timestamped transition into a mission phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseTransition {
    /// When the transition occurred.
    pub at: Timestamp,
    /// Phase the run moved into.
    pub phase: MissionPhase,
}

/// A single execution of a mission, with its identity and headline metadata.
///
/// The full picture of a run — its events, telemetry samples, assertion
/// results, and failure report — lives in sibling collections keyed by
/// [`MissionRunId`]. Issue 2 will introduce a storage layer that ties them
/// together; issue 1 keeps the bundle in memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRun {
    /// Identifier of this mission run.
    pub id: MissionRunId,
    /// Vehicle label (e.g. `"ardupilot_copter"`, `"px4_quad"`, `"demo"`).
    pub vehicle: String,
    /// Mission label (e.g. `"simple_takeoff"`).
    pub mission: String,
    /// How the run was produced.
    pub source: MissionSource,
    /// Current status.
    pub status: MissionStatus,
    /// Failure reason, present iff `status == Failed`.
    pub failure_reason: Option<FailureReason>,
    /// When the run started.
    pub started_at: Timestamp,
    /// When the run ended, if it has ended.
    pub ended_at: Option<Timestamp>,
    /// Ordered phase transitions that form the mission timeline.
    pub phases: Vec<PhaseTransition>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn phase_round_trips() {
        for phase in [
            MissionPhase::Disconnected,
            MissionPhase::Connected,
            MissionPhase::Arming,
            MissionPhase::Armed,
            MissionPhase::TakeoffRequested,
            MissionPhase::Climbing,
            MissionPhase::TargetAltitudeReached,
            MissionPhase::NavigatingToWaypoint,
            MissionPhase::ReturningHome,
            MissionPhase::Landing,
            MissionPhase::Landed,
            MissionPhase::Failed,
        ] {
            let json = serde_json::to_string(&phase).unwrap();
            let back: MissionPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(back, phase);
        }
    }

    #[test]
    fn mission_run_round_trips() {
        let run = MissionRun {
            id: MissionRunId::from_u128(0x01),
            vehicle: "demo".into(),
            mission: "simple_takeoff".into(),
            source: MissionSource::Demo,
            status: MissionStatus::Failed,
            failure_reason: Some(FailureReason::TakeoffTimeout),
            started_at: Timestamp(datetime!(2025-01-01 00:00:00 UTC)),
            ended_at: Some(Timestamp(datetime!(2025-01-01 00:00:12.5 UTC))),
            phases: vec![PhaseTransition {
                at: Timestamp(datetime!(2025-01-01 00:00:00 UTC)),
                phase: MissionPhase::Connected,
            }],
        };
        let json = serde_json::to_string(&run).unwrap();
        let back: MissionRun = serde_json::from_str(&json).unwrap();
        assert_eq!(back, run);
    }
}
