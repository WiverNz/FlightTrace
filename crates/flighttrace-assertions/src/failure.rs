//! Failure-report construction from the engine's per-check verdicts.
//!
//! Given a deterministic ordering of failed checks (first failing step, then
//! the canonical check order within that step), we pick one as the *primary*
//! cause. Its [`CheckKind`] determines the [`FailureReason`] and the
//! likely-cause paragraph; evidence from every failing check is concatenated
//! and deduplicated.

use uuid::Uuid;

use flighttrace_core::{
    Evidence, FailureReason, FailureReport, FailureReportId, MissionRun, NormalizedEvent,
    NormalizedEventKind, Timestamp,
};

use crate::engine::FailedCheck;

/// Namespace used to derive deterministic IDs for engine-produced artefacts.
///
/// Random UUID picked at issue 5 implementation; any stable value works as
/// long as it never changes (changing it would invalidate stored IDs).
const ID_NAMESPACE: Uuid = Uuid::from_u128(0x7f1e_8b62_4cad_4c40_9be3_2a4b_1ce1_5f00);

/// One of the v0.1 check kinds. Used to map a failing check to the right
/// [`FailureReason`] and to derive deterministic assertion result IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckKind {
    /// `heartbeat_within_sec`
    HeartbeatWithinSec,
    /// `armed`
    Armed,
    /// `altitude_at_least_m`
    AltitudeAtLeastM,
    /// `mode_not_changes`
    ModeNotChanges,
    /// `no_telemetry_gap_longer_than_sec`
    NoTelemetryGap,
}

impl CheckKind {
    /// Short identifier used in assertion result names and ID derivation.
    pub fn as_str(self) -> &'static str {
        match self {
            CheckKind::HeartbeatWithinSec => "heartbeat_within_sec",
            CheckKind::Armed => "armed",
            CheckKind::AltitudeAtLeastM => "altitude_at_least_m",
            CheckKind::ModeNotChanges => "mode_not_changes",
            CheckKind::NoTelemetryGap => "no_telemetry_gap_longer_than_sec",
        }
    }

    /// Deterministic in-step ordering — used to break ties when several checks
    /// in the same step fail.
    pub fn priority(self) -> u8 {
        match self {
            CheckKind::HeartbeatWithinSec => 0,
            CheckKind::Armed => 1,
            CheckKind::AltitudeAtLeastM => 2,
            CheckKind::ModeNotChanges => 3,
            CheckKind::NoTelemetryGap => 4,
        }
    }
}

/// Map a failing check to a canonical [`FailureReason`].
///
/// `armed` is the only context-sensitive case: if the engine sees any
/// command activity for the run, the failure looks like a CommandTimeout;
/// otherwise we have no evidence to attribute it and fall back to `Unknown`.
pub fn reason_for_failure(kind: CheckKind, events: &[NormalizedEvent]) -> FailureReason {
    match kind {
        CheckKind::HeartbeatWithinSec => FailureReason::NoHeartbeat,
        CheckKind::Armed => {
            let saw_command = events.iter().any(|e| {
                matches!(
                    e.kind,
                    NormalizedEventKind::CommandAccepted { .. }
                        | NormalizedEventKind::CommandRejected { .. }
                )
            });
            if saw_command {
                FailureReason::CommandTimeout
            } else {
                FailureReason::Unknown
            }
        }
        CheckKind::AltitudeAtLeastM => FailureReason::TakeoffTimeout,
        CheckKind::ModeNotChanges => FailureReason::UnexpectedModeChange,
        CheckKind::NoTelemetryGap => FailureReason::TelemetryGap,
    }
}

/// Static likely-cause string for the failure report's `likely_cause` field.
///
/// Wording for `TakeoffTimeout` is reproduced verbatim from `docs/prd.md`.
pub fn likely_cause_for(reason: FailureReason) -> &'static str {
    match reason {
        FailureReason::NoHeartbeat => {
            "No HEARTBEAT was received within the expected window. \
             Check the link, the vehicle's MAVLink stream, and the listener endpoint."
        }
        FailureReason::CommandTimeout => {
            "A command was sent but never acknowledged. \
             Check vehicle responsiveness, link quality, and command preconditions."
        }
        FailureReason::TakeoffTimeout => {
            "The vehicle stopped climbing before reaching target altitude. \
             Check estimator health, takeoff parameters, or mission timeout threshold."
        }
        FailureReason::WaypointNotReached => {
            "The vehicle did not reach the next waypoint in time. \
             Check planned route, vehicle dynamics, and timeout thresholds."
        }
        FailureReason::UnexpectedModeChange => {
            "The flight mode changed during a step where it was expected to stay constant. \
             Check operator inputs, failsafes, and autopilot state machine."
        }
        FailureReason::TelemetryGap => {
            "Telemetry stopped flowing for longer than the configured budget. \
             Check radio link, ground station, and listener wiring."
        }
        FailureReason::LowBattery => {
            "The battery dropped below the safe threshold. \
             Check pack health, mission duration, and reserve thresholds."
        }
        FailureReason::GpsUnhealthy => {
            "GPS solution degraded below the required quality. \
             Check satellite count, HDOP, and antenna placement."
        }
        FailureReason::Unknown => {
            "The mission failed but no specific cause could be attributed. \
             Inspect the event timeline for clues."
        }
    }
}

/// Derive a deterministic [`FailureReportId`] from a run id.
pub fn report_id_for(run_id: flighttrace_core::MissionRunId) -> FailureReportId {
    let name = format!("failure_report:{}", run_id.into_inner());
    let uuid = Uuid::new_v5(&ID_NAMESPACE, name.as_bytes());
    FailureReportId::new(uuid)
}

/// Derive a deterministic [`flighttrace_core::AssertionResultId`] from
/// `(run_id, step_idx, check_kind)`.
pub fn assertion_id_for(
    run_id: flighttrace_core::MissionRunId,
    step_idx: usize,
    kind: CheckKind,
) -> flighttrace_core::AssertionResultId {
    let name = format!(
        "assertion:{}:{step_idx}:{}",
        run_id.into_inner(),
        kind.as_str()
    );
    let uuid = Uuid::new_v5(&ID_NAMESPACE, name.as_bytes());
    flighttrace_core::AssertionResultId::new(uuid)
}

/// Build a [`FailureReport`] from the engine's failing-check list, or return
/// `None` when nothing failed.
pub fn build_failure_report(
    run: &MissionRun,
    events: &[NormalizedEvent],
    failed: &[FailedCheck],
) -> Option<FailureReport> {
    let primary = failed.first()?;

    let reason = reason_for_failure(primary.kind, events);
    let summary = format!("Mission failed: {}", screaming_snake(reason));
    let likely_cause = likely_cause_for(reason).to_string();

    let mut evidence: Vec<Evidence> = Vec::new();
    for failure in failed {
        for item in &failure.output.evidence {
            if !evidence.iter().any(|existing| existing == item) {
                evidence.push(item.clone());
            }
        }
    }

    let generated_at = generated_at_for(run, failed);

    Some(FailureReport {
        id: report_id_for(run.id),
        run_id: run.id,
        reason,
        summary,
        evidence,
        likely_cause,
        generated_at,
    })
}

/// Render a [`FailureReason`] in the API's SCREAMING_SNAKE_CASE form
/// (matches the `FailureReasonDto` JSON serialization in `flighttrace-api`).
fn screaming_snake(reason: FailureReason) -> &'static str {
    match reason {
        FailureReason::NoHeartbeat => "NO_HEARTBEAT",
        FailureReason::CommandTimeout => "COMMAND_TIMEOUT",
        FailureReason::TakeoffTimeout => "TAKEOFF_TIMEOUT",
        FailureReason::WaypointNotReached => "WAYPOINT_NOT_REACHED",
        FailureReason::UnexpectedModeChange => "UNEXPECTED_MODE_CHANGE",
        FailureReason::TelemetryGap => "TELEMETRY_GAP",
        FailureReason::LowBattery => "LOW_BATTERY",
        FailureReason::GpsUnhealthy => "GPS_UNHEALTHY",
        FailureReason::Unknown => "UNKNOWN",
    }
}

fn generated_at_for(run: &MissionRun, failed: &[FailedCheck]) -> Timestamp {
    // Prefer the run's ended_at when set, else the latest evaluated_at of any
    // failing check, else the run's started_at. Adding a tiny offset to
    // started_at would be misleading.
    if let Some(end) = run.ended_at {
        return end;
    }
    if let Some(latest) = failed
        .iter()
        .map(|f| f.output.evaluated_at)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
    {
        return latest;
    }
    run.started_at
}
