//! Parser tests: accept the documented format, reject malformed input.

use flighttrace_assertions::{AssertionError, parse_mission_yaml};

#[test]
fn accepts_simple_takeoff_yaml() {
    let yaml = include_str!("../../../examples/missions/simple_takeoff.yaml");
    let spec = parse_mission_yaml(yaml).expect("simple_takeoff.yaml must parse");
    assert_eq!(spec.mission, "simple_takeoff");
    assert_eq!(spec.vehicle, "ardupilot_copter");
    assert_eq!(spec.steps.len(), 3);
    assert_eq!(spec.steps[2].name, "takeoff");
    assert_eq!(spec.steps[2].expect.altitude_at_least_m, Some(10.0));
    assert_eq!(spec.steps[2].expect.within_sec, Some(30.0));
}

#[test]
fn rejects_unknown_top_level_field() {
    let yaml = r#"
mission: x
vehicle: y
something_else: nope
steps:
  - name: s
    expect:
      armed: true
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Parse(_)), "got {err:?}");
}

#[test]
fn rejects_unknown_expect_field() {
    let yaml = r#"
mission: x
vehicle: y
steps:
  - name: s
    expect:
      mode_not_change: true
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Parse(_)), "got {err:?}");
}

#[test]
fn rejects_missing_required_field() {
    let yaml = r#"
mission: x
steps:
  - name: s
    expect: {}
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Parse(_)), "got {err:?}");
}

#[test]
fn rejects_non_numeric_within_sec() {
    let yaml = r#"
mission: x
vehicle: y
steps:
  - name: s
    expect:
      within_sec: "soon"
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Parse(_)), "got {err:?}");
}

#[test]
fn rejects_empty_mission_name() {
    let yaml = r#"
mission: ""
vehicle: y
steps:
  - name: s
    expect:
      armed: true
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Invalid(_)), "got {err:?}");
}

#[test]
fn rejects_empty_steps_list() {
    let yaml = r#"
mission: x
vehicle: y
steps: []
"#;
    let err = parse_mission_yaml(yaml).unwrap_err();
    assert!(matches!(err, AssertionError::Invalid(_)), "got {err:?}");
}
