//! The deterministic `TAKEOFF_TIMEOUT` scenario.
//!
//! Everything in this module is fixed at compile time: the run identifier,
//! every event identifier, every timestamp offset, the altitude curve, the
//! assertion verdicts, and the failure report text. There is no randomness
//! and no clock dependency.

use time::Duration;
use time::OffsetDateTime;
use time::macros::datetime;

use flighttrace_core::{
    AssertionOutcome, AssertionResult, AssertionResultId, EventId, Evidence, FailureReason,
    FailureReport, FailureReportId, MissionPhase, MissionRun, MissionRunId, MissionSource,
    MissionStatus, NormalizedEvent, NormalizedEventKind, PhaseTransition, TelemetrySample,
    Timestamp,
};

use crate::DemoRun;

/// Epoch used as `t = 0` for the demo run.
const DEMO_EPOCH: OffsetDateTime = datetime!(2025-01-01 00:00:00 UTC);

/// Run id for the demo bundle (stable across calls).
const DEMO_RUN_ID: MissionRunId =
    MissionRunId::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

/// Failure report id for the demo bundle.
const DEMO_FAILURE_REPORT_ID: FailureReportId =
    FailureReportId::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0002);

/// Target altitude the assertion expects (and the vehicle never reaches).
const TARGET_ALTITUDE_M: f64 = 10.0;

/// Hard-coded telemetry samples: `(seconds_from_epoch, altitude_m, vertical_speed_mps)`.
///
/// Altitude rises smoothly from 0 m, peaks at 2.4 m, then stalls. The values
/// are tuned so the failure looks like an estimator/takeoff problem rather
/// than a clean climb.
const TELEMETRY: &[(f64, f64, f64)] = &[
    (3.0, 0.0, 0.6),
    (3.5, 0.3, 0.8),
    (4.0, 0.7, 0.8),
    (4.5, 1.1, 0.8),
    (5.0, 1.5, 0.6),
    (5.5, 1.8, 0.4),
    (6.0, 2.0, 0.4),
    (6.5, 2.2, 0.2),
    (7.0, 2.3, 0.2),
    (7.5, 2.4, 0.0),
    (8.0, 2.4, 0.0),
    (8.5, 2.4, 0.0),
    (9.0, 2.4, 0.0),
    (9.5, 2.4, 0.0),
    (10.0, 2.4, 0.0),
];

/// Convert a fixed offset (in seconds) from the demo epoch to a [`Timestamp`].
fn at(seconds: f64) -> Timestamp {
    let micros = (seconds * 1_000_000.0).round() as i64;
    Timestamp(DEMO_EPOCH + Duration::microseconds(micros))
}

/// Build the deterministic failed-takeoff demo bundle.
pub fn build() -> DemoRun {
    let mut events: Vec<NormalizedEvent> = Vec::new();
    // Counter-based deterministic event identifiers, starting at 0x0100.
    let mut next_event_id: u128 = 0x0100;
    let mut push_event =
        |events: &mut Vec<NormalizedEvent>, at_offset: f64, kind: NormalizedEventKind| -> EventId {
            let id = EventId::from_u128(next_event_id);
            next_event_id += 1;
            let event = NormalizedEvent {
                id,
                run_id: DEMO_RUN_ID,
                at: at(at_offset),
                kind,
            };
            events.push(event);
            id
        };

    // Pre-takeoff events.
    push_event(&mut events, 0.0, NormalizedEventKind::VehicleConnected);
    push_event(&mut events, 0.5, NormalizedEventKind::HeartbeatReceived);
    push_event(
        &mut events,
        1.0,
        NormalizedEventKind::ModeChanged {
            mode: "GUIDED".into(),
        },
    );
    push_event(
        &mut events,
        2.0,
        NormalizedEventKind::ArmedChanged { armed: true },
    );
    push_event(
        &mut events,
        2.5,
        NormalizedEventKind::CommandAccepted {
            command: "MAV_CMD_NAV_TAKEOFF".into(),
        },
    );

    // Climbing events: one AltitudeUpdated per telemetry sample, plus the
    // STATUSTEXT warning at t+7.5s.
    let stall_offset = 7.5;
    let mut emitted_statustext = false;
    let mut statustext_event_id: Option<EventId> = None;
    for &(offset, altitude_m, _vs) in TELEMETRY {
        if !emitted_statustext && offset >= stall_offset {
            let id = push_event(
                &mut events,
                stall_offset,
                NormalizedEventKind::StatustextReceived {
                    severity: 4,
                    text: "EKF altitude variance".into(),
                },
            );
            statustext_event_id = Some(id);
            emitted_statustext = true;
        }
        push_event(
            &mut events,
            offset,
            NormalizedEventKind::AltitudeUpdated { altitude_m },
        );
    }

    // Assertion + failure events.
    let assertion_failed_event_id = push_event(
        &mut events,
        12.0,
        NormalizedEventKind::AssertionFailed {
            assertion_name: "takeoff_reaches_10m".into(),
            detail: "max_altitude=2.4m target=10.0m".into(),
        },
    );
    let mission_failed_event_id = push_event(
        &mut events,
        12.5,
        NormalizedEventKind::MissionFailed {
            reason: FailureReason::TakeoffTimeout,
        },
    );

    // Telemetry samples — one per ~500 ms in the climbing window.
    let telemetry: Vec<TelemetrySample> = TELEMETRY
        .iter()
        .map(
            |&(offset, altitude_m, vertical_speed_mps)| TelemetrySample {
                run_id: DEMO_RUN_ID,
                at: at(offset),
                altitude_m,
                vertical_speed_mps,
                mode: "GUIDED".into(),
                armed: true,
                battery_remaining_pct: 85.0,
            },
        )
        .collect();

    // Peak / stall altitude for evidence and assertion detail.
    let max_altitude = telemetry
        .iter()
        .map(|s| s.altitude_m)
        .fold(f64::MIN, f64::max);

    // Phase transitions — the mission timeline.
    let phases = vec![
        PhaseTransition {
            at: at(0.0),
            phase: MissionPhase::Connected,
        },
        PhaseTransition {
            at: at(2.0),
            phase: MissionPhase::Armed,
        },
        PhaseTransition {
            at: at(2.5),
            phase: MissionPhase::TakeoffRequested,
        },
        PhaseTransition {
            at: at(3.0),
            phase: MissionPhase::Climbing,
        },
        PhaseTransition {
            at: at(12.5),
            phase: MissionPhase::Failed,
        },
    ];

    let started_at = at(0.0);
    let ended_at = at(12.5);

    let run = MissionRun {
        id: DEMO_RUN_ID,
        vehicle: "demo".into(),
        mission: "simple_takeoff".into(),
        source: MissionSource::Demo,
        status: MissionStatus::Failed,
        failure_reason: Some(FailureReason::TakeoffTimeout),
        started_at,
        ended_at: Some(ended_at),
        phases,
    };

    // Assertion results — hard-coded in issue 1; issue 5 will replace these
    // with output from `flighttrace-assertions` while keeping the same shape.
    let assertions = vec![
        AssertionResult {
            id: AssertionResultId::from_u128(0x0010),
            run_id: DEMO_RUN_ID,
            name: "heartbeat_within_5s".into(),
            outcome: AssertionOutcome::Pass,
            detail: "Heartbeat received 0.5s after connect.".into(),
            evaluated_at: at(0.5),
        },
        AssertionResult {
            id: AssertionResultId::from_u128(0x0011),
            run_id: DEMO_RUN_ID,
            name: "armed_within_5s".into(),
            outcome: AssertionOutcome::Pass,
            detail: "Vehicle armed 2.0s after connect.".into(),
            evaluated_at: at(2.0),
        },
        AssertionResult {
            id: AssertionResultId::from_u128(0x0012),
            run_id: DEMO_RUN_ID,
            name: "takeoff_reaches_10m".into(),
            outcome: AssertionOutcome::Fail,
            detail: format!("max_altitude={max_altitude:.1}m target={TARGET_ALTITUDE_M:.1}m"),
            evaluated_at: at(12.0),
        },
    ];

    // Failure report — the headline artefact. Evidence cites concrete events
    // and the stall sample.
    let mut evidence: Vec<Evidence> = Vec::new();
    evidence.push(Evidence::Note("Takeoff command was accepted.".into()));
    evidence.push(Evidence::Note("Vehicle armed successfully.".into()));
    evidence.push(Evidence::Note(format!(
        "Target altitude was {TARGET_ALTITUDE_M:.1} m."
    )));
    evidence.push(Evidence::Note(format!(
        "Maximum reached altitude was only {max_altitude:.1} m."
    )));
    evidence.push(Evidence::Note(
        "Vertical speed dropped near zero before reaching target altitude.".into(),
    ));
    evidence.push(Evidence::Note("Last known mode: GUIDED.".into()));
    if let Some(id) = statustext_event_id {
        evidence.push(Evidence::Event(id));
    }
    evidence.push(Evidence::Statustext {
        at: at(stall_offset),
        text: "EKF altitude variance".into(),
    });
    evidence.push(Evidence::Sample {
        at: at(stall_offset),
        field: "altitude_m".into(),
        value: format!("{max_altitude:.1}"),
    });
    evidence.push(Evidence::Event(assertion_failed_event_id));
    evidence.push(Evidence::Event(mission_failed_event_id));

    let failure_report = FailureReport {
        id: DEMO_FAILURE_REPORT_ID,
        run_id: DEMO_RUN_ID,
        reason: FailureReason::TakeoffTimeout,
        summary: "Mission failed: TAKEOFF_TIMEOUT".into(),
        evidence,
        likely_cause: "The vehicle stopped climbing before reaching target altitude. \
             Check estimator health, takeoff parameters, or mission timeout threshold."
            .into(),
        generated_at: ended_at,
    };

    DemoRun {
        run,
        events,
        telemetry,
        assertions,
        failure_report,
    }
}
