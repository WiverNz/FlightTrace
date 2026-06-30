// FlightTrace MAVLink ingestion is strictly read-only. Do not add send paths.
//
// This file owns the UDP listen loop. Every call into the `mavlink` crate
// here is `recv` / `recv_raw`. There is no `send` or `send_default` anywhere.
// If a future change adds telemetry forwarding, command upload, parameter
// writes, or any other "talk back to the vehicle" feature it must live in a
// separate, gated crate; this one is the trust boundary.

//! Tokio-based MAVLink UDP listener that drives ingestion into FlightTrace
//! storage.

use std::future::Future;
use std::time::Instant;

use sqlx::SqlitePool;
use tokio::time::{Interval, MissedTickBehavior, interval};
use tracing::{debug, info, warn};
use uuid::Uuid;

use flighttrace_assertions::{EvaluationInput, MissionSpec, evaluate};
use flighttrace_core::{
    EventId, MissionPhase, MissionRun, MissionRunId, MissionSource, MissionStatus, NormalizedEvent,
    NormalizedEventKind, PhaseTransition, TelemetrySample, Timestamp,
};
use flighttrace_storage::{
    AssertionResultRepo, FailureReportRepo, MissionRunRepo, NormalizedEventRepo,
    TelemetrySampleRepo,
};

use mavlink::dialects::common::MavMessage;

use crate::config::MavlinkConfig;
use crate::error::{MavlinkIngestError, MavlinkIngestResult};
use crate::gap::check_gap;
use crate::session::SessionState;
use crate::translate::{
    translate_battery_status, translate_command_ack, translate_global_position, translate_gps_raw,
    translate_heartbeat, translate_mission_current, translate_statustext, translate_sys_status,
    translate_vfr_hud, vehicle_label,
};

/// One in-flight ingestion session (1:1 with a `MissionRun`).
struct ActiveSession {
    run_id: MissionRunId,
    state: SessionState,
}

/// Run the MAVLink ingestion loop until `shutdown` resolves.
///
/// The listener:
/// - binds the configured UDP endpoint as a server (`udpin:`),
/// - waits for the first HEARTBEAT, then creates a `MissionRun`,
/// - translates each supported message into normalized events + a pending
///   telemetry sample,
/// - flushes telemetry on a `telemetry_batch_interval` cadence,
/// - emits a single `TelemetryGapDetected` per outage,
/// - closes the session (status = Succeeded unless `MissionFailed`) on idle
///   timeout or shutdown.
///
/// The listener never sends MAVLink traffic.
pub async fn run_ingestion(
    pool: SqlitePool,
    config: MavlinkConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> MavlinkIngestResult<()> {
    let address = config.mavlink_connect_address();
    info!(endpoint = %config.endpoint, address = %address, dialect = config.dialect.name(), "MAVLink ingestion starting");

    let conn = mavlink::connect_async::<MavMessage>(&address)
        .await
        .map_err(MavlinkIngestError::Connect)?;
    info!(endpoint = %config.endpoint, "MAVLink ingestion bound; waiting for HEARTBEAT");

    let mut session: Option<ActiveSession> = None;
    let mut ticker: Interval = interval(config.telemetry_batch_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut shutdown = Box::pin(shutdown);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => {
                info!("MAVLink ingestion shutdown signal received");
                break;
            }
            _ = ticker.tick() => {
                if let Some(active) = session.as_mut() {
                    if let Err(err) = on_tick(active, &pool, &config).await {
                        warn!(%err, "MAVLink tick handler error");
                    }
                    if should_close_for_idle(active, &config) {
                        info!(run = %active.run_id, "MAVLink session idle timeout; closing");
                        let active = session.take().expect("session must be Some here");
                        close_session(active, &pool, config.mission.as_ref()).await?;
                    }
                }
            }
            recv_result = conn.recv() => {
                match recv_result {
                    Ok((_, msg)) => {
                        if let Err(err) =
                            on_message(msg, &pool, &config, &mut session).await
                        {
                            warn!(%err, "MAVLink message handler error");
                        }
                    }
                    Err(err) => {
                        debug!(%err, "MAVLink recv error (ignored)");
                    }
                }
            }
        }
    }

    if let Some(active) = session.take() {
        close_session(active, &pool, config.mission.as_ref()).await?;
    }
    info!("MAVLink ingestion stopped");
    Ok(())
}

async fn on_message(
    msg: MavMessage,
    pool: &SqlitePool,
    config: &MavlinkConfig,
    session: &mut Option<ActiveSession>,
) -> MavlinkIngestResult<()> {
    let now_instant = Instant::now();
    let now_ts = Timestamp(time::OffsetDateTime::now_utc());

    // First HEARTBEAT bootstraps a session.
    if session.is_none() {
        let MavMessage::HEARTBEAT(hb) = &msg else {
            // Wait for the first heartbeat before persisting anything.
            return Ok(());
        };
        let run_id = MissionRunId::new(Uuid::new_v4());
        let vehicle = vehicle_label(hb);
        let run = MissionRun {
            id: run_id,
            vehicle: vehicle.clone(),
            mission: "live".to_string(),
            source: MissionSource::Mavlink,
            status: MissionStatus::Running,
            failure_reason: None,
            started_at: now_ts,
            ended_at: None,
            phases: vec![PhaseTransition {
                at: now_ts,
                phase: MissionPhase::Connected,
            }],
        };
        MissionRunRepo::new(pool).insert(&run).await?;
        info!(run = %run_id, %vehicle, "MAVLink session opened");
        let mut state = SessionState::default();
        state.note_any(now_instant);
        *session = Some(ActiveSession { run_id, state });
    }

    let active = session.as_mut().expect("session opened above");
    let run_id = active.run_id;

    let events: Vec<NormalizedEventKind> = match msg {
        MavMessage::HEARTBEAT(ref hb) => {
            active.state.note_any(now_instant);
            translate_heartbeat(hb, &mut active.state)
        }
        MavMessage::GLOBAL_POSITION_INT(ref m) => {
            active.state.note_telemetry(now_instant);
            translate_global_position(m, &mut active.state)
        }
        MavMessage::VFR_HUD(ref m) => {
            active.state.note_telemetry(now_instant);
            translate_vfr_hud(m, &mut active.state)
        }
        MavMessage::ATTITUDE(_) => {
            active.state.note_telemetry(now_instant);
            Vec::new()
        }
        MavMessage::SYS_STATUS(ref m) => {
            active.state.note_telemetry(now_instant);
            translate_sys_status(m, &mut active.state)
        }
        MavMessage::BATTERY_STATUS(ref m) => {
            active.state.note_telemetry(now_instant);
            translate_battery_status(m, &mut active.state)
        }
        MavMessage::GPS_RAW_INT(ref m) => {
            active.state.note_telemetry(now_instant);
            translate_gps_raw(m, &mut active.state)
        }
        MavMessage::MISSION_CURRENT(ref m) => {
            active.state.note_any(now_instant);
            translate_mission_current(m, &mut active.state)
        }
        MavMessage::STATUSTEXT(ref m) => {
            active.state.note_any(now_instant);
            translate_statustext(m, &mut active.state)
        }
        MavMessage::COMMAND_ACK(ref m) => {
            active.state.note_any(now_instant);
            translate_command_ack(m, &mut active.state)
        }
        _ => {
            active.state.note_any(now_instant);
            Vec::new()
        }
    };

    persist_events(pool, run_id, now_ts, &events, &mut active.state).await?;

    // Driving the gap detector on every message catches the recovery edge.
    let _ = check_gap(
        &mut active.state,
        now_instant,
        config.telemetry_gap_threshold,
    );

    Ok(())
}

async fn on_tick(
    active: &mut ActiveSession,
    pool: &SqlitePool,
    config: &MavlinkConfig,
) -> MavlinkIngestResult<()> {
    let now_instant = Instant::now();
    let now_ts = Timestamp(time::OffsetDateTime::now_utc());

    // 1. Gap detection: emit a single event per outage.
    if let Some(ev) = check_gap(
        &mut active.state,
        now_instant,
        config.telemetry_gap_threshold,
    ) {
        let event = NormalizedEvent {
            id: EventId::new(Uuid::new_v4()),
            run_id: active.run_id,
            at: now_ts,
            kind: ev,
        };
        NormalizedEventRepo::new(pool).insert(&event).await?;
    }

    // 2. Flush pending telemetry sample.
    if let Some(sample) = active.state.pending_sample.flush(active.run_id, now_ts) {
        write_sample(pool, &sample).await?;
    }

    Ok(())
}

fn should_close_for_idle(active: &ActiveSession, config: &MavlinkConfig) -> bool {
    let Some(last) = active.state.last_any_message_at else {
        return false;
    };
    Instant::now().saturating_duration_since(last) > config.session_idle_timeout
}

async fn close_session(
    active: ActiveSession,
    pool: &SqlitePool,
    mission: Option<&MissionSpec>,
) -> MavlinkIngestResult<()> {
    let ended_at = Timestamp(time::OffsetDateTime::now_utc());
    let run_id = active.run_id;
    let mission_failed = active.state.mission_failed;

    // Without a mission spec we keep the issue-4 behaviour: classify purely
    // from the in-band MissionFailed flag.
    let Some(mission) = mission else {
        let (status, failure_reason) = if mission_failed {
            (MissionStatus::Failed, None)
        } else {
            (MissionStatus::Succeeded, None)
        };
        finish_run(pool, run_id, status, failure_reason, ended_at).await?;
        return Ok(());
    };

    // First write the headline `ended_at` so the engine sees a closed window.
    MissionRunRepo::new(pool)
        .update_status(run_id, MissionStatus::Running, None, Some(ended_at))
        .await?;

    let run = match MissionRunRepo::new(pool).get(run_id).await? {
        Some(run) => run,
        None => {
            warn!(run = %run_id, "session close: run row missing");
            return Ok(());
        }
    };
    let events = NormalizedEventRepo::new(pool).list_for_run(run_id).await?;
    let samples = TelemetrySampleRepo::new(pool).list_for_run(run_id).await?;

    let output = evaluate(
        mission,
        &EvaluationInput {
            run: &run,
            events: &events,
            samples: &samples,
        },
    );

    AssertionResultRepo::new(pool)
        .replace_for_run(run_id, &output.results)
        .await?;

    let failure_repo = FailureReportRepo::new(pool);
    let (status, failure_reason) = match output.failure_report.as_ref() {
        Some(report) => {
            failure_repo.upsert_for_run(report).await?;
            (MissionStatus::Failed, Some(report.reason))
        }
        None => {
            failure_repo.delete_for_run(run_id).await?;
            if mission_failed {
                // Engine cleared the run but the vehicle still signalled
                // failure in-band; trust the vehicle and keep the run Failed.
                (MissionStatus::Failed, None)
            } else {
                (MissionStatus::Succeeded, None)
            }
        }
    };
    finish_run(pool, run_id, status, failure_reason, ended_at).await?;
    Ok(())
}

async fn finish_run(
    pool: &SqlitePool,
    run_id: MissionRunId,
    status: MissionStatus,
    failure_reason: Option<flighttrace_core::FailureReason>,
    ended_at: Timestamp,
) -> MavlinkIngestResult<()> {
    let updated = MissionRunRepo::new(pool)
        .update_status(run_id, status, failure_reason, Some(ended_at))
        .await?;
    if !updated {
        warn!(run = %run_id, "session close: run row missing");
    } else {
        info!(run = %run_id, ?status, "MAVLink session closed");
    }
    Ok(())
}

async fn persist_events(
    pool: &SqlitePool,
    run_id: MissionRunId,
    at: Timestamp,
    kinds: &[NormalizedEventKind],
    state: &mut SessionState,
) -> MavlinkIngestResult<()> {
    if kinds.is_empty() {
        return Ok(());
    }
    let repo = NormalizedEventRepo::new(pool);
    for kind in kinds {
        if let NormalizedEventKind::MissionFailed { reason } = kind {
            state.note_mission_failure(*reason);
        }
        let event = NormalizedEvent {
            id: EventId::new(Uuid::new_v4()),
            run_id,
            at,
            kind: kind.clone(),
        };
        repo.insert(&event).await?;
    }
    Ok(())
}

async fn write_sample(pool: &SqlitePool, sample: &TelemetrySample) -> MavlinkIngestResult<()> {
    TelemetrySampleRepo::new(pool).insert(sample).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use mavlink::dialects::common::{HEARTBEAT_DATA, MavAutopilot, MavModeFlag, MavState, MavType};

    use crate::config::Dialect;
    use flighttrace_storage::connect_in_memory;

    fn cfg() -> MavlinkConfig {
        let mut c = MavlinkConfig::new("udp://127.0.0.1:0", Dialect::Common).unwrap();
        c.telemetry_batch_interval = Duration::from_millis(50);
        c.telemetry_gap_threshold = Duration::from_millis(200);
        c.session_idle_timeout = Duration::from_millis(500);
        c
    }

    fn heartbeat(armed: bool, custom_mode: u32) -> HEARTBEAT_DATA {
        let mut base_mode = MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED;
        if armed {
            base_mode |= MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED;
        }
        HEARTBEAT_DATA {
            custom_mode,
            mavtype: MavType::MAV_TYPE_QUADROTOR,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            base_mode,
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        }
    }

    #[tokio::test]
    async fn first_heartbeat_creates_running_run() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(false, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();

        let active = session.as_ref().expect("session opened");
        let run = MissionRunRepo::new(&pool)
            .get(active.run_id)
            .await
            .unwrap()
            .expect("run row");
        assert_eq!(run.source, MissionSource::Mavlink);
        assert_eq!(run.status, MissionStatus::Running);
        assert_eq!(run.mission, "live");
        assert_eq!(run.vehicle, "ardupilot_quad");
        assert!(run.ended_at.is_none());

        let events = NormalizedEventRepo::new(&pool)
            .list_for_run(active.run_id)
            .await
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e.kind, NormalizedEventKind::VehicleConnected))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e.kind, NormalizedEventKind::HeartbeatReceived))
        );
    }

    #[tokio::test]
    async fn close_session_sets_status_to_succeeded_when_no_failure() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(false, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();
        let active = session.take().unwrap();
        let run_id = active.run_id;
        close_session(active, &pool, None).await.unwrap();

        let run = MissionRunRepo::new(&pool)
            .get(run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, MissionStatus::Succeeded);
        assert!(run.ended_at.is_some());
    }

    #[tokio::test]
    async fn close_session_sets_status_to_failed_when_mission_failed_event_seen() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(false, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();
        let active = session.as_mut().unwrap();
        // Simulate a downstream assertion engine recording a MissionFailed
        // for this run (issue 5 will write these for real; here we just flip
        // the in-memory flag the listener watches).
        active
            .state
            .note_mission_failure(flighttrace_core::FailureReason::TakeoffTimeout);

        let active = session.take().unwrap();
        let run_id = active.run_id;
        close_session(active, &pool, None).await.unwrap();
        let run = MissionRunRepo::new(&pool)
            .get(run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, MissionStatus::Failed);
    }

    #[tokio::test]
    async fn telemetry_tick_writes_pending_sample() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(true, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();

        let gpos = mavlink::dialects::common::GLOBAL_POSITION_INT_DATA {
            time_boot_ms: 0,
            lat: 0,
            lon: 0,
            alt: 0,
            relative_alt: 4_500, // 4.5 m
            vx: 0,
            vy: 0,
            vz: -100, // 1 m/s up
            hdg: 0,
        };
        on_message(
            MavMessage::GLOBAL_POSITION_INT(gpos),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();

        let active = session.as_mut().unwrap();
        let run_id = active.run_id;
        on_tick(active, &pool, &cfg).await.unwrap();

        let samples = TelemetrySampleRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap();
        assert_eq!(samples.len(), 1);
        assert!((samples[0].altitude_m - 4.5).abs() < 1e-6);
        assert!((samples[0].vertical_speed_mps - 1.0).abs() < 1e-6);
        assert_eq!(samples[0].mode, "GUIDED");
        assert!(samples[0].armed);
    }

    #[tokio::test]
    async fn close_session_with_mission_writes_assertions_and_succeeds_for_clean_flight() {
        use flighttrace_assertions::parse_mission_yaml;
        use flighttrace_storage::{AssertionResultRepo, FailureReportRepo};

        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(true, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();
        let active = session.take().unwrap();
        let run_id = active.run_id;

        // A minimal spec that this synthetic session trivially satisfies: a
        // single heartbeat keeps the heartbeat check within budget.
        let spec = parse_mission_yaml(
            "mission: live\nvehicle: any\nsteps:\n  - name: warmup\n    expect:\n      heartbeat_within_sec: 60\n",
        )
        .unwrap();
        close_session(active, &pool, Some(&spec)).await.unwrap();

        let run = MissionRunRepo::new(&pool)
            .get(run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, MissionStatus::Succeeded);
        assert!(run.ended_at.is_some());

        let results = AssertionResultRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap();
        assert_eq!(results.len(), 1, "one check, one result");
        assert_eq!(results[0].outcome, flighttrace_core::AssertionOutcome::Pass);

        let report = FailureReportRepo::new(&pool)
            .get_for_run(run_id)
            .await
            .unwrap();
        assert!(report.is_none(), "no failure → no report");
    }

    #[tokio::test]
    async fn close_session_with_mission_records_failure_report_when_check_fails() {
        use flighttrace_assertions::parse_mission_yaml;
        use flighttrace_storage::{AssertionResultRepo, FailureReportRepo};

        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(true, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();
        let active = session.take().unwrap();
        let run_id = active.run_id;

        // Demand altitude ≥ 10m within 1s — the synthetic session has no
        // altitude data, so the check fails and the engine writes a
        // TAKEOFF_TIMEOUT failure report.
        let spec = parse_mission_yaml(
            "mission: live\nvehicle: any\nsteps:\n  - name: takeoff\n    expect:\n      altitude_at_least_m: 10\n      within_sec: 1\n",
        )
        .unwrap();
        close_session(active, &pool, Some(&spec)).await.unwrap();

        let run = MissionRunRepo::new(&pool)
            .get(run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.status, MissionStatus::Failed);
        assert_eq!(
            run.failure_reason,
            Some(flighttrace_core::FailureReason::TakeoffTimeout)
        );

        let results = AssertionResultRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, flighttrace_core::AssertionOutcome::Fail);

        let report = FailureReportRepo::new(&pool)
            .get_for_run(run_id)
            .await
            .unwrap()
            .expect("failure report persisted");
        assert_eq!(
            report.reason,
            flighttrace_core::FailureReason::TakeoffTimeout
        );
        assert_eq!(report.summary, "Mission failed: TAKEOFF_TIMEOUT");
    }

    #[tokio::test]
    async fn idle_timeout_detection_returns_true_after_threshold() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = cfg();
        let mut session: Option<ActiveSession> = None;
        on_message(
            MavMessage::HEARTBEAT(heartbeat(false, 4)),
            &pool,
            &cfg,
            &mut session,
        )
        .await
        .unwrap();
        let active = session.as_mut().unwrap();
        // Backdate last_any_message_at past the threshold.
        active.state.last_any_message_at =
            Some(Instant::now() - cfg.session_idle_timeout - Duration::from_secs(1));
        assert!(should_close_for_idle(active, &cfg));
    }
}
