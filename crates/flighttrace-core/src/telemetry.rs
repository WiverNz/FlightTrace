//! Telemetry samples.
//!
//! A telemetry sample is a periodic numeric snapshot of vehicle state.
//! v0.1 intentionally keeps the shape narrow — the fields here are the ones
//! the demo, the assertion engine, and the failure-report UI need.

use serde::{Deserialize, Serialize};

use crate::ids::MissionRunId;
use crate::time::Timestamp;

/// One periodic snapshot of vehicle state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySample {
    /// Mission run this sample belongs to.
    pub run_id: MissionRunId,
    /// When the sample was recorded.
    pub at: Timestamp,
    /// Altitude in metres (relative to home unless the source specifies AMSL).
    pub altitude_m: f64,
    /// Vertical speed in metres per second, positive = up.
    pub vertical_speed_mps: f64,
    /// Current flight mode (autopilot-specific string).
    pub mode: String,
    /// Whether the vehicle is armed.
    pub armed: bool,
    /// Estimated battery remaining as a percentage `0..=100`.
    pub battery_remaining_pct: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn telemetry_sample_round_trips() {
        let sample = TelemetrySample {
            run_id: MissionRunId::from_u128(0x01),
            at: Timestamp(datetime!(2025-01-01 00:00:03 UTC)),
            altitude_m: 1.5,
            vertical_speed_mps: 0.6,
            mode: "GUIDED".into(),
            armed: true,
            battery_remaining_pct: 85.0,
        };
        let json = serde_json::to_string(&sample).unwrap();
        let back: TelemetrySample = serde_json::from_str(&json).unwrap();
        assert_eq!(back, sample);
    }
}
