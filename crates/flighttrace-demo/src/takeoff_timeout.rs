//! The deterministic `TAKEOFF_TIMEOUT` scenario.
//!
//! Events, telemetry samples, and run metadata are all fixed at compile time;
//! the assertion verdicts and the failure report come from the real engine
//! (`flighttrace-assertions`) parsing `examples/missions/simple_takeoff.yaml`
//! and evaluating it against this curated bundle. There is no randomness and
//! no clock dependency.

use time::Duration;
use time::OffsetDateTime;
use time::macros::datetime;

use flighttrace_assertions::{EvaluationInput, evaluate, parse_mission_yaml};
use flighttrace_core::{
    EventId, FailureReason, MissionPhase, MissionRun, MissionRunId, MissionSource, MissionStatus,
    NormalizedEvent, NormalizedEventKind, PhaseTransition, TelemetrySample, Timestamp,
};

use crate::DemoRun;

/// YAML mission spec the demo evaluates against.
const SIMPLE_TAKEOFF_YAML: &str = include_str!("../../../examples/missions/simple_takeoff.yaml");

/// Epoch used as `t = 0` for the demo run.
const DEMO_EPOCH: OffsetDateTime = datetime!(2025-01-01 00:00:00 UTC);

/// Run id for the demo bundle (stable across calls).
const DEMO_RUN_ID: MissionRunId =
    MissionRunId::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

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
    for &(offset, altitude_m, _vs) in TELEMETRY {
        if !emitted_statustext && offset >= stall_offset {
            push_event(
                &mut events,
                stall_offset,
                NormalizedEventKind::StatustextReceived {
                    severity: 4,
                    text: "EKF altitude variance".into(),
                },
            );
            emitted_statustext = true;
        }
        push_event(
            &mut events,
            offset,
            NormalizedEventKind::AltitudeUpdated { altitude_m },
        );
    }

    // The vehicle (autopilot-equivalent in this demo) signals mission failure
    // at t+12.5s. The assertion engine independently produces the matching
    // FailureReport from the bundle below.
    push_event(
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

    // Run the real assertion engine against the bundle. The engine produces
    // deterministic IDs for results and the failure report, so two calls
    // yield byte-identical JSON.
    let spec = parse_mission_yaml(SIMPLE_TAKEOFF_YAML)
        .expect("bundled simple_takeoff.yaml must always parse");
    let input = EvaluationInput {
        run: &run,
        events: &events,
        samples: &telemetry,
    };
    let evaluation = evaluate(&spec, &input);
    let failure_report = evaluation
        .failure_report
        .expect("takeoff bundle must produce a failure report");

    DemoRun {
        run,
        events,
        telemetry,
        assertions: evaluation.results,
        failure_report,
    }
}
