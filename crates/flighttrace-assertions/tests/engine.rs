//! End-to-end engine tests against hand-built synthetic bundles.

use time::Duration;
use time::macros::datetime;

use flighttrace_assertions::{EvaluationInput, evaluate, parse_mission_yaml};
use flighttrace_core::{
    AssertionOutcome, EventId, Evidence, FailureReason, MissionPhase, MissionRun, MissionRunId,
    MissionSource, MissionStatus, NormalizedEvent, NormalizedEventKind, PhaseTransition,
    TelemetrySample, Timestamp,
};

const RUN_ID: MissionRunId = MissionRunId::from_u128(0x01);

fn t0() -> Timestamp {
    Timestamp(datetime!(2025-01-01 00:00:00 UTC))
}

fn at(secs: f64) -> Timestamp {
    Timestamp(t0().0 + Duration::seconds_f64(secs))
}

fn event(seq: u128, secs: f64, kind: NormalizedEventKind) -> NormalizedEvent {
    NormalizedEvent {
        id: EventId::from_u128(seq),
        run_id: RUN_ID,
        at: at(secs),
        kind,
    }
}

fn failed_takeoff_run() -> MissionRun {
    MissionRun {
        id: RUN_ID,
        vehicle: "demo".into(),
        mission: "simple_takeoff".into(),
        source: MissionSource::Demo,
        status: MissionStatus::Failed,
        failure_reason: Some(FailureReason::TakeoffTimeout),
        started_at: at(0.0),
        ended_at: Some(at(12.5)),
        phases: vec![PhaseTransition {
            at: at(0.0),
            phase: MissionPhase::Connected,
        }],
    }
}

fn failed_takeoff_events() -> Vec<NormalizedEvent> {
    let mut events = vec![
        event(0x10, 0.0, NormalizedEventKind::VehicleConnected),
        event(0x11, 0.5, NormalizedEventKind::HeartbeatReceived),
        event(
            0x12,
            1.0,
            NormalizedEventKind::ModeChanged {
                mode: "GUIDED".into(),
            },
        ),
        event(0x13, 2.0, NormalizedEventKind::ArmedChanged { armed: true }),
        event(
            0x14,
            2.5,
            NormalizedEventKind::CommandAccepted {
                command: "MAV_CMD_NAV_TAKEOFF".into(),
            },
        ),
    ];
    let mut seq = 0x20;
    let altitudes = [
        (3.0, 0.0),
        (4.0, 0.7),
        (5.0, 1.5),
        (6.0, 2.0),
        (7.0, 2.3),
        (7.5, 2.4),
        (8.0, 2.4),
        (10.0, 2.4),
    ];
    for (t, alt) in altitudes {
        events.push(event(
            seq,
            t,
            NormalizedEventKind::AltitudeUpdated { altitude_m: alt },
        ));
        seq += 1;
    }
    events.push(event(
        seq,
        7.5,
        NormalizedEventKind::StatustextReceived {
            severity: 4,
            text: "EKF altitude variance".into(),
        },
    ));
    events
}

fn failed_takeoff_samples() -> Vec<TelemetrySample> {
    [(3.0, 0.0), (5.0, 1.5), (7.5, 2.4), (10.0, 2.4)]
        .iter()
        .map(|&(t, alt)| TelemetrySample {
            run_id: RUN_ID,
            at: at(t),
            altitude_m: alt,
            vertical_speed_mps: 0.0,
            mode: "GUIDED".into(),
            armed: true,
            battery_remaining_pct: 85.0,
        })
        .collect()
}

#[test]
fn takeoff_timeout_bundle_produces_takeoff_timeout_report() {
    let yaml = include_str!("../../../examples/missions/simple_takeoff.yaml");
    let spec = parse_mission_yaml(yaml).unwrap();

    let run = failed_takeoff_run();
    let events = failed_takeoff_events();
    let samples = failed_takeoff_samples();
    let input = EvaluationInput {
        run: &run,
        events: &events,
        samples: &samples,
    };

    let output = evaluate(&spec, &input);

    // Three steps × the checks we declared = 5 assertion results
    // (connect.heartbeat + arm.armed + takeoff.{altitude, mode, gap}).
    assert_eq!(output.results.len(), 5);

    // At least one pass and one fail.
    assert!(
        output
            .results
            .iter()
            .any(|r| matches!(r.outcome, AssertionOutcome::Pass))
    );
    let altitude_fail = output
        .results
        .iter()
        .find(|r| r.name == "takeoff.altitude_at_least_m")
        .expect("takeoff altitude check should be present");
    assert_eq!(altitude_fail.outcome, AssertionOutcome::Fail);
    assert!(altitude_fail.detail.contains("2.4"));

    let report = output
        .failure_report
        .as_ref()
        .expect("failed bundle must produce a failure report");
    assert_eq!(report.reason, FailureReason::TakeoffTimeout);
    assert_eq!(report.summary, "Mission failed: TAKEOFF_TIMEOUT");
    assert!(report.likely_cause.to_lowercase().contains("altitude"));
    assert!(!report.evidence.is_empty());
}

#[test]
fn telemetry_gap_bundle_produces_telemetry_gap_report() {
    let yaml = r#"
mission: gap_run
vehicle: demo
steps:
  - name: cruise
    expect:
      no_telemetry_gap_longer_than_sec: 2
      within_sec: 60
"#;
    let spec = parse_mission_yaml(yaml).unwrap();

    let run = MissionRun {
        id: RUN_ID,
        vehicle: "demo".into(),
        mission: "gap_run".into(),
        source: MissionSource::Mavlink,
        status: MissionStatus::Failed,
        failure_reason: None,
        started_at: at(0.0),
        ended_at: Some(at(60.0)),
        phases: vec![],
    };
    let gap_event_id = EventId::from_u128(0xDEAD_BEEF);
    let events = vec![
        event(1, 0.0, NormalizedEventKind::VehicleConnected),
        event(2, 0.5, NormalizedEventKind::HeartbeatReceived),
        NormalizedEvent {
            id: gap_event_id,
            run_id: RUN_ID,
            at: at(10.0),
            kind: NormalizedEventKind::TelemetryGapDetected { gap_sec: 5.0 },
        },
    ];
    let input = EvaluationInput {
        run: &run,
        events: &events,
        samples: &[],
    };

    let output = evaluate(&spec, &input);
    let report = output
        .failure_report
        .expect("gap bundle must produce a failure report");
    assert_eq!(report.reason, FailureReason::TelemetryGap);
    assert_eq!(report.summary, "Mission failed: TELEMETRY_GAP");
    assert!(
        report
            .evidence
            .iter()
            .any(|e| matches!(e, Evidence::Event(id) if *id == gap_event_id)),
        "expected the gap event to be cited as evidence; got {:?}",
        report.evidence,
    );
}

#[test]
fn successful_bundle_has_no_failure_report() {
    let yaml = r#"
mission: ok
vehicle: demo
steps:
  - name: connect
    expect:
      heartbeat_within_sec: 5
  - name: takeoff
    expect:
      altitude_at_least_m: 1
      within_sec: 30
"#;
    let spec = parse_mission_yaml(yaml).unwrap();
    let run = MissionRun {
        id: RUN_ID,
        vehicle: "demo".into(),
        mission: "ok".into(),
        source: MissionSource::Demo,
        status: MissionStatus::Succeeded,
        failure_reason: None,
        started_at: at(0.0),
        ended_at: Some(at(30.0)),
        phases: vec![],
    };
    let events = vec![
        event(1, 1.0, NormalizedEventKind::HeartbeatReceived),
        event(
            2,
            5.0,
            NormalizedEventKind::AltitudeUpdated { altitude_m: 2.0 },
        ),
    ];
    let input = EvaluationInput {
        run: &run,
        events: &events,
        samples: &[],
    };
    let output = evaluate(&spec, &input);
    assert!(output.failure_report.is_none());
    assert!(
        output
            .results
            .iter()
            .all(|r| matches!(r.outcome, AssertionOutcome::Pass))
    );
}

#[test]
fn evaluation_is_deterministic() {
    let yaml = include_str!("../../../examples/missions/simple_takeoff.yaml");
    let spec = parse_mission_yaml(yaml).unwrap();
    let run = failed_takeoff_run();
    let events = failed_takeoff_events();
    let samples = failed_takeoff_samples();
    let input = EvaluationInput {
        run: &run,
        events: &events,
        samples: &samples,
    };
    let a = evaluate(&spec, &input);
    let b = evaluate(&spec, &input);
    let ids_a: Vec<_> = a.results.iter().map(|r| r.id).collect();
    let ids_b: Vec<_> = b.results.iter().map(|r| r.id).collect();
    assert_eq!(ids_a, ids_b);
    assert_eq!(
        a.failure_report.as_ref().map(|r| r.id),
        b.failure_report.as_ref().map(|r| r.id),
    );
}
