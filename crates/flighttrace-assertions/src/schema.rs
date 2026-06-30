//! Serde structs that mirror the mission YAML format.
//!
//! The schema is deliberately small: one top-level [`MissionSpec`] containing
//! a list of [`StepSpec`]s, each of which carries an [`ExpectSpec`] with the
//! v0.1 check fields. Unknown keys at any level are rejected by serde so a
//! typo such as `mode_not_change` (missing trailing `s`) fails at parse time
//! rather than being silently ignored.

use serde::{Deserialize, Serialize};

/// A full mission specification loaded from YAML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionSpec {
    /// Mission label (matches `MissionRun.mission`).
    pub mission: String,
    /// Vehicle label (matches `MissionRun.vehicle`). Informational only.
    pub vehicle: String,
    /// Ordered list of mission steps.
    pub steps: Vec<StepSpec>,
}

/// One step inside a [`MissionSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepSpec {
    /// Human-readable step name (e.g. `"takeoff"`).
    pub name: String,
    /// Checks to evaluate for this step.
    pub expect: ExpectSpec,
}

/// Bag of v0.1 checks attached to one step. Each field is independent and
/// optional; the engine evaluates exactly the checks that are present.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectSpec {
    /// Maximum acceptable time (seconds) between two consecutive HEARTBEATs
    /// during the step's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_within_sec: Option<f64>,

    /// Whether the vehicle must be armed (`true`) or disarmed (`false`)
    /// within the step's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armed: Option<bool>,

    /// Minimum altitude (metres) the vehicle must reach within `within_sec`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude_at_least_m: Option<f64>,

    /// Step-level deadline (seconds, relative to the run's `started_at`) that
    /// time-bounded checks apply within. Without this, time-bounded checks
    /// consider the full run window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within_sec: Option<f64>,

    /// When `true`, the flight mode must not change during the step's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_not_changes: Option<bool>,

    /// Maximum acceptable telemetry gap (seconds) within the step's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_telemetry_gap_longer_than_sec: Option<f64>,
}
