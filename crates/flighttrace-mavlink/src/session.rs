//! In-memory session state for a single MAVLink ingestion session.
//!
//! A session corresponds 1:1 with a `MissionRun`. It tracks the values we've
//! already observed so we can suppress duplicate events (e.g. emit
//! `ArmedChanged` only when the bit actually flips) and keeps a "pending"
//! telemetry sample fragment that the listener flushes on its 250 ms cadence.

use std::time::Instant;

use flighttrace_core::{FailureReason, MissionRunId, TelemetrySample, Timestamp};

/// What we know about the vehicle so far.
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    /// True after we emit the first `VehicleConnected` (post-first-heartbeat).
    pub vehicle_connected_emitted: bool,
    /// Last `armed` value we observed (None until the first HEARTBEAT).
    pub last_armed: Option<bool>,
    /// Last mode string we observed.
    pub last_mode: Option<String>,
    /// Last battery percentage we emitted; suppresses duplicate `BatteryUpdated`s.
    pub last_battery_remaining_pct: Option<f32>,
    /// Last battery voltage we observed.
    pub last_battery_voltage_v: Option<f32>,
    /// Last waypoint index we observed.
    pub last_waypoint: Option<u32>,
    /// True if any `MissionFailed` event has been emitted for this session.
    pub mission_failed: bool,
    /// Carried-through cumulative sample fragment used for the next flush.
    pub pending_sample: PendingSample,
    /// `Instant` at which the last telemetry-bearing message arrived. Used by
    /// the gap detector.
    pub last_telemetry_message_at: Option<Instant>,
    /// `Instant` at which we last emitted a `TelemetryGapDetected` event. Used
    /// to suppress duplicate gap events while the stream is still down.
    pub gap_emitted_at: Option<Instant>,
    /// `Instant` of the last HEARTBEAT (any message, technically) used to
    /// drive the session-idle timeout.
    pub last_any_message_at: Option<Instant>,
}

impl SessionState {
    /// Reset state for a new session.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Record a telemetry-bearing message arrival (used by the gap detector
    /// and the idle-timeout watchdog).
    pub fn note_telemetry(&mut self, now: Instant) {
        self.last_telemetry_message_at = Some(now);
        self.last_any_message_at = Some(now);
        self.gap_emitted_at = None;
    }

    /// Record any-message arrival (HEARTBEAT, COMMAND_ACK, etc.). Resets the
    /// session-idle countdown but not the gap detector.
    pub fn note_any(&mut self, now: Instant) {
        self.last_any_message_at = Some(now);
    }

    /// Mark that we observed a `MissionFailed` event.
    pub fn note_mission_failure(&mut self, _reason: FailureReason) {
        self.mission_failed = true;
    }
}

/// Telemetry-sample fragment that the listener flushes on a fixed cadence.
///
/// Each MAVLink message contributes the fields it can. Fields that no message
/// has yet provided fall back to sensible defaults at flush time (altitude=0,
/// vertical_speed=0, mode="UNKNOWN", armed=false, battery=100.0).
#[derive(Debug, Clone, Default)]
pub struct PendingSample {
    /// Altitude in metres above home (from GLOBAL_POSITION_INT.relative_alt or VFR_HUD.alt).
    pub altitude_m: Option<f64>,
    /// Vertical speed in m/s, positive = up.
    pub vertical_speed_mps: Option<f64>,
    /// Flight mode string.
    pub mode: Option<String>,
    /// Armed flag.
    pub armed: Option<bool>,
    /// Battery remaining percentage.
    pub battery_remaining_pct: Option<f32>,
    /// True if any field was set since the last flush.
    pub dirty: bool,
}

impl PendingSample {
    /// Mark the fragment as dirty.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Take the fragment and materialise a [`TelemetrySample`] using `at` as
    /// the timestamp. Returns `None` if no field has ever been set.
    pub fn flush(&mut self, run_id: MissionRunId, at: Timestamp) -> Option<TelemetrySample> {
        if !self.dirty {
            return None;
        }
        let sample = TelemetrySample {
            run_id,
            at,
            altitude_m: self.altitude_m.unwrap_or(0.0),
            vertical_speed_mps: self.vertical_speed_mps.unwrap_or(0.0),
            mode: self.mode.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
            armed: self.armed.unwrap_or(false),
            battery_remaining_pct: self.battery_remaining_pct.unwrap_or(100.0),
        };
        self.dirty = false;
        Some(sample)
    }
}
