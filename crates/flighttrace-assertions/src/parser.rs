//! YAML → [`MissionSpec`] conversion.

use std::fs;
use std::path::Path;

use crate::error::{AssertionError, AssertionResult};
use crate::schema::MissionSpec;

/// Parse a YAML document into a [`MissionSpec`].
///
/// Unknown top-level or per-step keys are rejected (schema uses
/// `deny_unknown_fields`), so typos in check names fail loudly.
pub fn parse_mission_yaml(yaml: &str) -> AssertionResult<MissionSpec> {
    let spec: MissionSpec = serde_yaml::from_str(yaml)?;
    validate(&spec)?;
    Ok(spec)
}

/// Read a YAML mission file from disk and parse it.
pub fn load_mission_file(path: &Path) -> AssertionResult<MissionSpec> {
    let raw = fs::read_to_string(path).map_err(|source| AssertionError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_mission_yaml(&raw)
}

fn validate(spec: &MissionSpec) -> AssertionResult<()> {
    if spec.mission.trim().is_empty() {
        return Err(AssertionError::Invalid("mission name is empty".into()));
    }
    if spec.vehicle.trim().is_empty() {
        return Err(AssertionError::Invalid("vehicle is empty".into()));
    }
    if spec.steps.is_empty() {
        return Err(AssertionError::Invalid("mission has no steps".into()));
    }
    for (idx, step) in spec.steps.iter().enumerate() {
        if step.name.trim().is_empty() {
            return Err(AssertionError::Invalid(format!(
                "step #{idx} has an empty name"
            )));
        }
    }
    Ok(())
}
