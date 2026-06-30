//! Per-check unit coverage: each check has a passing and a failing case.

use time::Duration;
use time::macros::datetime;

use flighttrace_assertions::checks::{
    Window, altitude::check_altitude_at_least_m, armed::check_armed,
    gap::check_no_telemetry_gap_longer_than_sec, heartbeat::check_heartbeat_within_sec,
    mode::check_mode_not_changes,
};
use flighttrace_core::{
    AssertionOutcome, EventId, MissionRunId, NormalizedEvent, NormalizedEventKind, TelemetrySample,
    Timestamp,
};

const RUN_ID: MissionRunId = MissionRunId::from_u128(0xAA);

fn t0() -> Timestamp {
    Timestamp(datetime!(2025-01-01 00:00:00 UTC))
}

fn at(secs: f64) -> Timestamp {
    Timestamp(t0().0 + Duration::seconds_f64(secs))
}

fn window(end_offset_sec: f64) -> Window {
    Window {
        start: t0(),
        end: at(end_offset_sec),
    }
}

fn event(seq: u128, offset: f64, kind: NormalizedEventKind) -> NormalizedEvent {
    NormalizedEvent {
        id: EventId::from_u128(seq),
        run_id: RUN_ID,
        at: at(offset),
        kind,
    }
}

#[test]
fn heartbeat_pass_when_within_budget() {
    let events = vec![
        event(1, 0.5, NormalizedEventKind::HeartbeatReceived),
        event(2, 2.0, NormalizedEventKind::HeartbeatReceived),
    ];
    let out = check_heartbeat_within_sec(5.0, window(10.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
    assert!(!out.evidence.is_empty());
}

#[test]
fn heartbeat_fail_when_first_arrives_late() {
    let events = vec![event(1, 6.0, NormalizedEventKind::HeartbeatReceived)];
    let out = check_heartbeat_within_sec(5.0, window(10.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
}

#[test]
fn heartbeat_fail_when_no_heartbeat_in_window() {
    let events = vec![event(1, 1.0, NormalizedEventKind::VehicleConnected)];
    let out = check_heartbeat_within_sec(5.0, window(10.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
}

#[test]
fn armed_pass_when_armed_event_in_window() {
    let events = vec![event(
        1,
        2.0,
        NormalizedEventKind::ArmedChanged { armed: true },
    )];
    let out = check_armed(true, window(5.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
}

#[test]
fn armed_fail_when_no_event_in_window() {
    let events: Vec<NormalizedEvent> = vec![];
    let out = check_armed(true, window(5.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
}

#[test]
fn altitude_pass_when_threshold_reached_by_event() {
    let events = vec![event(
        1,
        4.0,
        NormalizedEventKind::AltitudeUpdated { altitude_m: 12.0 },
    )];
    let out = check_altitude_at_least_m(10.0, window(30.0), &events, &[]);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
}

#[test]
fn altitude_pass_when_threshold_reached_by_sample() {
    let samples = vec![TelemetrySample {
        run_id: RUN_ID,
        at: at(5.0),
        altitude_m: 11.0,
        vertical_speed_mps: 1.0,
        mode: "GUIDED".into(),
        armed: true,
        battery_remaining_pct: 90.0,
    }];
    let out = check_altitude_at_least_m(10.0, window(30.0), &[], &samples);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
}

#[test]
fn altitude_fail_when_threshold_never_reached() {
    let events = vec![
        event(
            1,
            3.0,
            NormalizedEventKind::AltitudeUpdated { altitude_m: 1.5 },
        ),
        event(
            2,
            6.0,
            NormalizedEventKind::AltitudeUpdated { altitude_m: 2.4 },
        ),
    ];
    let out = check_altitude_at_least_m(10.0, window(30.0), &events, &[]);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
    assert!(out.detail.contains("2.4"));
    assert!(out.detail.contains("10.0"));
}

#[test]
fn mode_pass_when_mode_stable() {
    let events = vec![
        event(
            1,
            1.0,
            NormalizedEventKind::ModeChanged {
                mode: "GUIDED".into(),
            },
        ),
        event(
            2,
            5.0,
            NormalizedEventKind::ModeChanged {
                mode: "GUIDED".into(),
            },
        ),
    ];
    let out = check_mode_not_changes(true, window(30.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
}

#[test]
fn mode_fail_when_mode_changes() {
    let events = vec![
        event(
            1,
            1.0,
            NormalizedEventKind::ModeChanged {
                mode: "GUIDED".into(),
            },
        ),
        event(
            2,
            6.0,
            NormalizedEventKind::ModeChanged {
                mode: "LOITER".into(),
            },
        ),
    ];
    let out = check_mode_not_changes(true, window(30.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
}

#[test]
fn gap_pass_when_no_gap_event() {
    let out = check_no_telemetry_gap_longer_than_sec(2.0, window(30.0), &[]);
    assert_eq!(out.outcome, AssertionOutcome::Pass);
}

#[test]
fn gap_fail_when_gap_exceeds_budget() {
    let events = vec![event(
        1,
        10.0,
        NormalizedEventKind::TelemetryGapDetected { gap_sec: 5.0 },
    )];
    let out = check_no_telemetry_gap_longer_than_sec(2.0, window(30.0), &events);
    assert_eq!(out.outcome, AssertionOutcome::Fail);
}
