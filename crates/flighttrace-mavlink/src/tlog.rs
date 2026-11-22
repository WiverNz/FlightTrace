// FlightTrace MAVLink ingestion is strictly read-only. Do not add send paths.
//
// This file replays a recorded `.tlog` file. It only reads bytes from disk.

//! Import a recorded `.tlog` file (QGroundControl, Mission Planner, MAVProxy)
//! into FlightTrace as one `MissionRun`.
//!
//! Frames are read with [`mavlink_tlog::TlogReader`], which checks every
//! frame's CRC and skips damaged bytes instead of decoding them, then fed
//! through the same translation and assertion path as live ingestion, on the
//! log's own timestamps.

use std::io::Read;
use std::time::{Duration, Instant};

use mavlink::dialects::common::{MavMessage, MavType};
use mavlink::{MavlinkVersion, Message};
use mavlink_tlog::{TlogReader, Version};
use sqlx::SqlitePool;
use time::OffsetDateTime;
use tracing::info;

use flighttrace_core::{MissionRunId, Timestamp};

use crate::config::MavlinkConfig;
use crate::error::{MavlinkIngestError, MavlinkIngestResult};
use crate::listener::{ActiveSession, close_session_at, on_message_at, on_tick_at};

/// Replay a `.tlog` stream into `pool` and evaluate `config.mission` on it.
///
/// The vehicle is the first sender whose HEARTBEAT is not a ground station;
/// frames from other senders (GCS heartbeats, companions) are ignored.
/// Returns the created run, or `None` when the log has no vehicle HEARTBEAT.
pub async fn import_tlog<R: Read>(
    pool: &SqlitePool,
    reader: R,
    config: &MavlinkConfig,
) -> MavlinkIngestResult<Option<MissionRunId>> {
    let tick = config
        .telemetry_batch_interval
        .max(Duration::from_millis(1));
    let base = Instant::now();
    let mut log = TlogReader::new(reader);
    let mut session: Option<ActiveSession> = None;
    let mut vehicle: Option<u8> = None;
    let mut first_us: Option<u64> = None;
    let mut next_tick = Duration::ZERO;
    let mut last = (base, Timestamp(OffsetDateTime::now_utc()));

    while let Some(rec) = log.next_record().map_err(MavlinkIngestError::Tlog)? {
        let version = match rec.frame.version() {
            Version::V1 => MavlinkVersion::V1,
            Version::V2 => MavlinkVersion::V2,
        };
        let Ok(msg) = MavMessage::parse(version, rec.frame.msg_id(), rec.frame.payload()) else {
            continue;
        };
        let sys_id = rec.frame.sys_id();
        match vehicle {
            Some(id) if id != sys_id => continue,
            Some(_) => {}
            None => match &msg {
                MavMessage::HEARTBEAT(hb) if hb.mavtype != MavType::MAV_TYPE_GCS => {
                    vehicle = Some(sys_id)
                }
                _ => continue,
            },
        }

        let offset =
            Duration::from_micros(rec.timestamp_us - *first_us.get_or_insert(rec.timestamp_us));
        let at = (base + offset, log_time(rec.timestamp_us));

        if let Some(active) = session.as_mut() {
            while next_tick <= offset {
                let tick_ts = Timestamp(at.1.0 - (offset - next_tick));
                on_tick_at(active, base + next_tick, tick_ts, pool, config).await?;
                next_tick += tick;
            }
        } else {
            next_tick = offset + tick;
        }

        on_message_at(msg, at.0, at.1, pool, config, &mut session).await?;
        last = at;
    }

    let Some(mut active) = session else {
        return Ok(None);
    };
    on_tick_at(&mut active, last.0, last.1, pool, config).await?;
    let run_id = active.run_id;
    let stats = log.stats();
    info!(run = %run_id, records = stats.records, skipped_bytes = stats.skipped_bytes, "tlog imported");
    close_session_at(active, pool, config.mission.as_ref(), last.1).await?;
    Ok(Some(run_id))
}

/// Wall-clock time of a tlog record (microseconds since the Unix epoch).
fn log_time(timestamp_us: u64) -> Timestamp {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_us) * 1_000)
        .map(Timestamp)
        .unwrap_or_else(|_| Timestamp(OffsetDateTime::UNIX_EPOCH))
}

#[cfg(test)]
mod tests {
    use super::*;

    use mavlink::MavHeader;
    use mavlink::dialects::common::{
        GLOBAL_POSITION_INT_DATA, HEARTBEAT_DATA, MavAutopilot, MavModeFlag, MavState,
    };
    use mavlink_tlog::TlogWriter;

    use flighttrace_assertions::parse_mission_yaml;
    use flighttrace_core::{AssertionOutcome, MissionStatus};
    use flighttrace_storage::{AssertionResultRepo, MissionRunRepo, connect_in_memory};

    use crate::config::Dialect;

    const T0_US: u64 = 1_700_000_000_000_000;

    fn heartbeat(mavtype: MavType) -> MavMessage {
        MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: 4,
            mavtype,
            autopilot: MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
            base_mode: MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED
                | MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED,
            system_status: MavState::MAV_STATE_ACTIVE,
            mavlink_version: 3,
        })
    }

    fn altitude(relative_alt_mm: i32) -> MavMessage {
        MavMessage::GLOBAL_POSITION_INT(GLOBAL_POSITION_INT_DATA {
            time_boot_ms: 0,
            lat: 0,
            lon: 0,
            alt: 0,
            relative_alt: relative_alt_mm,
            vx: 0,
            vy: 0,
            vz: -200,
            hdg: 0,
        })
    }

    /// A recorded flight: GCS heartbeat, vehicle heartbeat, climb to 12 m
    /// over 3 s, plus a few bytes of line noise mid-log.
    fn recorded_flight() -> Vec<u8> {
        let mut log = TlogWriter::new(Vec::new());
        let mut put = |at_ms: u64, system_id: u8, msg: MavMessage| {
            let header = MavHeader {
                system_id,
                component_id: 1,
                sequence: 0,
            };
            let mut frame = Vec::new();
            mavlink::write_v2_msg(&mut frame, header, &msg).unwrap();
            log.write_frame(T0_US + at_ms * 1_000, &frame).unwrap();
        };
        put(0, 255, heartbeat(MavType::MAV_TYPE_GCS));
        put(100, 1, heartbeat(MavType::MAV_TYPE_QUADROTOR));
        put(1_000, 1, altitude(4_000));
        put(2_000, 1, altitude(8_000));
        put(3_000, 1, altitude(12_000));
        put(3_500, 1, heartbeat(MavType::MAV_TYPE_QUADROTOR));
        let mut bytes = log.into_inner();
        // Line noise between the vehicle HEARTBEAT and the first position record.
        bytes.splice(58..58, [0xFD, 0x09, 0x00, 0x00]);
        bytes
    }

    #[tokio::test]
    async fn tlog_replays_through_assertions_on_log_time() {
        let pool = connect_in_memory().await.unwrap();
        let mut cfg = MavlinkConfig::new("udp://127.0.0.1:0", Dialect::Common).unwrap();
        cfg.telemetry_batch_interval = Duration::from_millis(500);
        cfg.mission = Some(
            parse_mission_yaml(
                "mission: replay\nvehicle: any\nsteps:\n  - name: takeoff\n    expect:\n      altitude_at_least_m: 10\n      within_sec: 5\n",
            )
            .unwrap(),
        );

        let run_id = import_tlog(&pool, recorded_flight().as_slice(), &cfg)
            .await
            .unwrap()
            .expect("vehicle heartbeat opens a run");

        let run = MissionRunRepo::new(&pool)
            .get(run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            run.vehicle, "ardupilot_quad",
            "GCS heartbeat is not the vehicle"
        );
        assert_eq!(run.started_at, log_time(T0_US + 100_000));
        assert_eq!(run.ended_at, Some(log_time(T0_US + 3_500_000)));
        assert_eq!(run.status, MissionStatus::Succeeded);

        let results = AssertionResultRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, AssertionOutcome::Pass);
    }

    #[tokio::test]
    async fn tlog_without_vehicle_heartbeat_creates_no_run() {
        let pool = connect_in_memory().await.unwrap();
        let cfg = MavlinkConfig::new("udp://127.0.0.1:0", Dialect::Common).unwrap();
        assert!(import_tlog(&pool, &[][..], &cfg).await.unwrap().is_none());
    }
}
