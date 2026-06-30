//! camelCase DTOs that mirror the core domain types at the API boundary.
//!
//! Domain types live in [`flighttrace_core`] and use Rust-idiomatic names.
//! These DTOs add `#[serde(rename_all = "camelCase")]` and translate the
//! handful of enums whose wire representation needs to be opinionated
//! (`FailureReason` is SCREAMING_SNAKE_CASE so the JSON value reads
//! `"TAKEOFF_TIMEOUT"`).
//!
//! Field-level docs are intentionally omitted: every field is a 1:1 mirror
//! of a field on the corresponding `flighttrace_core` type, where it is
//! already documented.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use flighttrace_core::{
    AssertionOutcome, AssertionResult, Evidence, FailureReason, FailureReport, MissionPhase,
    MissionRun, MissionSource, MissionStatus, NormalizedEvent, NormalizedEventKind,
    PhaseTransition, TelemetrySample, Timestamp,
};

/// Headline JSON for a mission run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionRunDto {
    pub id: String,
    pub vehicle: String,
    pub mission: String,
    pub source: MissionSourceDto,
    pub status: MissionStatusDto,
    pub failure_reason: Option<FailureReasonDto>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub phases: Vec<PhaseTransitionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseTransitionDto {
    pub at: Timestamp,
    pub phase: MissionPhaseDto,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MissionSourceDto {
    Demo,
    Mavlink,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MissionStatusDto {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MissionPhaseDto {
    Disconnected,
    Connected,
    Arming,
    Armed,
    TakeoffRequested,
    Climbing,
    TargetAltitudeReached,
    NavigatingToWaypoint,
    ReturningHome,
    Landing,
    Landed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureReasonDto {
    NoHeartbeat,
    CommandTimeout,
    TakeoffTimeout,
    WaypointNotReached,
    UnexpectedModeChange,
    TelemetryGap,
    LowBattery,
    GpsUnhealthy,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssertionOutcomeDto {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEventDto {
    pub id: String,
    pub run_id: String,
    pub at: Timestamp,
    #[serde(flatten)]
    pub kind: NormalizedEventKindDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum NormalizedEventKindDto {
    VehicleConnected,
    HeartbeatReceived,
    ArmedChanged {
        armed: bool,
    },
    ModeChanged {
        mode: String,
    },
    CommandAccepted {
        command: String,
    },
    CommandRejected {
        command: String,
        reason: String,
    },
    PositionUpdated {
        lat: f64,
        lon: f64,
    },
    AltitudeUpdated {
        altitude_m: f64,
    },
    BatteryUpdated {
        remaining_pct: f32,
        voltage_v: f32,
    },
    WaypointChanged {
        index: u32,
    },
    TelemetryGapDetected {
        gap_sec: f64,
    },
    StatustextReceived {
        severity: u8,
        text: String,
    },
    AssertionPassed {
        assertion_name: String,
    },
    AssertionFailed {
        assertion_name: String,
        detail: String,
    },
    MissionFailed {
        reason: FailureReasonDto,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySampleDto {
    pub run_id: String,
    pub at: Timestamp,
    pub altitude_m: f64,
    pub vertical_speed_mps: f64,
    pub mode: String,
    pub armed: bool,
    pub battery_remaining_pct: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssertionResultDto {
    pub id: String,
    pub run_id: String,
    pub name: String,
    pub outcome: AssertionOutcomeDto,
    pub detail: String,
    pub evaluated_at: Timestamp,
}

/// Request body for `POST /api/runs/:id/reevaluate`. Both fields are
/// optional; an empty body is equivalent to `{}` and means "use the run's
/// tagged mission file".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReevaluateBody {
    /// Filesystem path to the mission YAML to evaluate against. Overrides the
    /// run's tagged mission.
    pub mission_path: Option<String>,
}

/// Response body for `POST /api/runs/:id/reevaluate`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReevaluateResponse {
    pub assertions: Vec<AssertionResultDto>,
    pub failure_report: Option<FailureReportDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureReportDto {
    pub id: String,
    pub run_id: String,
    pub reason: FailureReasonDto,
    pub summary: String,
    pub evidence: Vec<EvidenceDto>,
    pub likely_cause: String,
    pub generated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
pub enum EvidenceDto {
    Note {
        text: String,
    },
    Event {
        event_id: String,
    },
    Sample {
        at: Timestamp,
        field: String,
        value: String,
    },
    Statustext {
        at: Timestamp,
        text: String,
    },
}

// -------------------------------------------------------------------------
// From-conversions
// -------------------------------------------------------------------------

impl From<MissionSource> for MissionSourceDto {
    fn from(value: MissionSource) -> Self {
        match value {
            MissionSource::Demo => Self::Demo,
            MissionSource::Mavlink => Self::Mavlink,
        }
    }
}

impl From<MissionStatus> for MissionStatusDto {
    fn from(value: MissionStatus) -> Self {
        match value {
            MissionStatus::Running => Self::Running,
            MissionStatus::Succeeded => Self::Succeeded,
            MissionStatus::Failed => Self::Failed,
        }
    }
}

impl From<MissionPhase> for MissionPhaseDto {
    fn from(value: MissionPhase) -> Self {
        match value {
            MissionPhase::Disconnected => Self::Disconnected,
            MissionPhase::Connected => Self::Connected,
            MissionPhase::Arming => Self::Arming,
            MissionPhase::Armed => Self::Armed,
            MissionPhase::TakeoffRequested => Self::TakeoffRequested,
            MissionPhase::Climbing => Self::Climbing,
            MissionPhase::TargetAltitudeReached => Self::TargetAltitudeReached,
            MissionPhase::NavigatingToWaypoint => Self::NavigatingToWaypoint,
            MissionPhase::ReturningHome => Self::ReturningHome,
            MissionPhase::Landing => Self::Landing,
            MissionPhase::Landed => Self::Landed,
            MissionPhase::Failed => Self::Failed,
        }
    }
}

impl From<FailureReason> for FailureReasonDto {
    fn from(value: FailureReason) -> Self {
        match value {
            FailureReason::NoHeartbeat => Self::NoHeartbeat,
            FailureReason::CommandTimeout => Self::CommandTimeout,
            FailureReason::TakeoffTimeout => Self::TakeoffTimeout,
            FailureReason::WaypointNotReached => Self::WaypointNotReached,
            FailureReason::UnexpectedModeChange => Self::UnexpectedModeChange,
            FailureReason::TelemetryGap => Self::TelemetryGap,
            FailureReason::LowBattery => Self::LowBattery,
            FailureReason::GpsUnhealthy => Self::GpsUnhealthy,
            FailureReason::Unknown => Self::Unknown,
        }
    }
}

impl From<AssertionOutcome> for AssertionOutcomeDto {
    fn from(value: AssertionOutcome) -> Self {
        match value {
            AssertionOutcome::Pass => Self::Pass,
            AssertionOutcome::Warn => Self::Warn,
            AssertionOutcome::Fail => Self::Fail,
        }
    }
}

impl From<&PhaseTransition> for PhaseTransitionDto {
    fn from(value: &PhaseTransition) -> Self {
        Self {
            at: value.at,
            phase: value.phase.into(),
        }
    }
}

impl From<MissionRun> for MissionRunDto {
    fn from(value: MissionRun) -> Self {
        Self {
            id: value.id.to_string(),
            vehicle: value.vehicle,
            mission: value.mission,
            source: value.source.into(),
            status: value.status.into(),
            failure_reason: value.failure_reason.map(Into::into),
            started_at: value.started_at,
            ended_at: value.ended_at,
            phases: value.phases.iter().map(Into::into).collect(),
        }
    }
}

impl From<&NormalizedEventKind> for NormalizedEventKindDto {
    fn from(value: &NormalizedEventKind) -> Self {
        match value {
            NormalizedEventKind::VehicleConnected => Self::VehicleConnected,
            NormalizedEventKind::HeartbeatReceived => Self::HeartbeatReceived,
            NormalizedEventKind::ArmedChanged { armed } => Self::ArmedChanged { armed: *armed },
            NormalizedEventKind::ModeChanged { mode } => Self::ModeChanged { mode: mode.clone() },
            NormalizedEventKind::CommandAccepted { command } => Self::CommandAccepted {
                command: command.clone(),
            },
            NormalizedEventKind::CommandRejected { command, reason } => Self::CommandRejected {
                command: command.clone(),
                reason: reason.clone(),
            },
            NormalizedEventKind::PositionUpdated { lat, lon } => Self::PositionUpdated {
                lat: *lat,
                lon: *lon,
            },
            NormalizedEventKind::AltitudeUpdated { altitude_m } => Self::AltitudeUpdated {
                altitude_m: *altitude_m,
            },
            NormalizedEventKind::BatteryUpdated {
                remaining_pct,
                voltage_v,
            } => Self::BatteryUpdated {
                remaining_pct: *remaining_pct,
                voltage_v: *voltage_v,
            },
            NormalizedEventKind::WaypointChanged { index } => {
                Self::WaypointChanged { index: *index }
            }
            NormalizedEventKind::TelemetryGapDetected { gap_sec } => {
                Self::TelemetryGapDetected { gap_sec: *gap_sec }
            }
            NormalizedEventKind::StatustextReceived { severity, text } => {
                Self::StatustextReceived {
                    severity: *severity,
                    text: text.clone(),
                }
            }
            NormalizedEventKind::AssertionPassed { assertion_name } => Self::AssertionPassed {
                assertion_name: assertion_name.clone(),
            },
            NormalizedEventKind::AssertionFailed {
                assertion_name,
                detail,
            } => Self::AssertionFailed {
                assertion_name: assertion_name.clone(),
                detail: detail.clone(),
            },
            NormalizedEventKind::MissionFailed { reason } => Self::MissionFailed {
                reason: (*reason).into(),
            },
        }
    }
}

impl From<NormalizedEvent> for NormalizedEventDto {
    fn from(value: NormalizedEvent) -> Self {
        Self {
            id: value.id.to_string(),
            run_id: value.run_id.to_string(),
            at: value.at,
            kind: (&value.kind).into(),
        }
    }
}

impl From<TelemetrySample> for TelemetrySampleDto {
    fn from(value: TelemetrySample) -> Self {
        Self {
            run_id: value.run_id.to_string(),
            at: value.at,
            altitude_m: value.altitude_m,
            vertical_speed_mps: value.vertical_speed_mps,
            mode: value.mode,
            armed: value.armed,
            battery_remaining_pct: value.battery_remaining_pct,
        }
    }
}

impl From<AssertionResult> for AssertionResultDto {
    fn from(value: AssertionResult) -> Self {
        Self {
            id: value.id.to_string(),
            run_id: value.run_id.to_string(),
            name: value.name,
            outcome: value.outcome.into(),
            detail: value.detail,
            evaluated_at: value.evaluated_at,
        }
    }
}

impl From<&Evidence> for EvidenceDto {
    fn from(value: &Evidence) -> Self {
        match value {
            Evidence::Note(text) => Self::Note { text: text.clone() },
            Evidence::Event(id) => Self::Event {
                event_id: id.to_string(),
            },
            Evidence::Sample { at, field, value } => Self::Sample {
                at: *at,
                field: field.clone(),
                value: value.clone(),
            },
            Evidence::Statustext { at, text } => Self::Statustext {
                at: *at,
                text: text.clone(),
            },
        }
    }
}

impl From<FailureReport> for FailureReportDto {
    fn from(value: FailureReport) -> Self {
        Self {
            id: value.id.to_string(),
            run_id: value.run_id.to_string(),
            reason: value.reason.into(),
            summary: value.summary,
            evidence: value.evidence.iter().map(Into::into).collect(),
            likely_cause: value.likely_cause,
            generated_at: value.generated_at,
        }
    }
}
