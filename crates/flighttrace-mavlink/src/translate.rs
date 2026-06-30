//! Pure translation from MAVLink messages to FlightTrace normalized events.
//!
//! Each `translate_*` function takes one MAVLink message + the [`SessionState`]
//! and returns the (possibly empty) list of [`NormalizedEventKind`]s that
//! should be persisted. Telemetry-sample fragments are written into
//! `session.pending_sample`; the listener is responsible for the actual flush.
//!
//! Translation is side-effect-free relative to the network and to storage:
//! these functions only touch the in-memory session.

use mavlink::dialects::common::{
    BATTERY_STATUS_DATA, COMMAND_ACK_DATA, GLOBAL_POSITION_INT_DATA, GPS_RAW_INT_DATA,
    HEARTBEAT_DATA, MISSION_CURRENT_DATA, MavModeFlag, MavResult, MavType, STATUSTEXT_DATA,
    SYS_STATUS_DATA, VFR_HUD_DATA,
};

use flighttrace_core::NormalizedEventKind;

use crate::session::SessionState;

/// Translate a HEARTBEAT into normalized events and update session mode/armed
/// state.
pub fn translate_heartbeat(
    msg: &HEARTBEAT_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let mut events = Vec::with_capacity(4);

    if !session.vehicle_connected_emitted {
        events.push(NormalizedEventKind::VehicleConnected);
        session.vehicle_connected_emitted = true;
    }

    events.push(NormalizedEventKind::HeartbeatReceived);

    let armed = msg
        .base_mode
        .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
    if session.last_armed != Some(armed) {
        events.push(NormalizedEventKind::ArmedChanged { armed });
        session.last_armed = Some(armed);
        session.pending_sample.armed = Some(armed);
        session.pending_sample.touch();
    }

    let mode = mode_string(msg);
    if session.last_mode.as_deref() != Some(mode.as_str()) {
        events.push(NormalizedEventKind::ModeChanged { mode: mode.clone() });
        session.last_mode = Some(mode.clone());
        session.pending_sample.mode = Some(mode);
        session.pending_sample.touch();
    }

    events
}

/// Translate GLOBAL_POSITION_INT into a `PositionUpdated` + `AltitudeUpdated`
/// plus telemetry-fragment updates.
pub fn translate_global_position(
    msg: &GLOBAL_POSITION_INT_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let lat = (msg.lat as f64) / 1e7;
    let lon = (msg.lon as f64) / 1e7;
    let altitude_m = (msg.relative_alt as f64) / 1000.0;
    // vz is "Ground Z Speed (positive down)" in cm/s. FlightTrace's
    // vertical_speed_mps is positive-up, so we negate.
    let vertical_speed_mps = -(msg.vz as f64) / 100.0;

    session.pending_sample.altitude_m = Some(altitude_m);
    session.pending_sample.vertical_speed_mps = Some(vertical_speed_mps);
    session.pending_sample.touch();

    vec![
        NormalizedEventKind::PositionUpdated { lat, lon },
        NormalizedEventKind::AltitudeUpdated { altitude_m },
    ]
}

/// Translate VFR_HUD; provides altitude (MSL) + climb rate. We prefer
/// GLOBAL_POSITION_INT's relative altitude when both arrive, but VFR_HUD's
/// `climb` is a more reliable vertical-speed source.
pub fn translate_vfr_hud(
    msg: &VFR_HUD_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    session.pending_sample.vertical_speed_mps = Some(msg.climb as f64);
    if session.pending_sample.altitude_m.is_none() {
        session.pending_sample.altitude_m = Some(msg.alt as f64);
    }
    session.pending_sample.touch();
    // No event by default — VFR_HUD just refines the telemetry sample.
    Vec::new()
}

/// Translate SYS_STATUS battery state.
pub fn translate_sys_status(
    msg: &SYS_STATUS_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let remaining_pct = if msg.battery_remaining < 0 {
        return Vec::new();
    } else {
        msg.battery_remaining as f32
    };
    let voltage_v = if msg.voltage_battery == u16::MAX {
        session.last_battery_voltage_v.unwrap_or(0.0)
    } else {
        (msg.voltage_battery as f32) / 1000.0
    };

    session.pending_sample.battery_remaining_pct = Some(remaining_pct);
    session.pending_sample.touch();

    let last = session.last_battery_remaining_pct;
    let changed = match last {
        None => true,
        Some(prev) => (prev - remaining_pct).abs() >= 1.0,
    };
    if !changed {
        return Vec::new();
    }
    session.last_battery_remaining_pct = Some(remaining_pct);
    session.last_battery_voltage_v = Some(voltage_v);
    vec![NormalizedEventKind::BatteryUpdated {
        remaining_pct,
        voltage_v,
    }]
}

/// Translate BATTERY_STATUS — same shape as SYS_STATUS but more detailed. We
/// use `battery_remaining` and `voltages[0]`.
pub fn translate_battery_status(
    msg: &BATTERY_STATUS_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    if msg.battery_remaining < 0 {
        return Vec::new();
    }
    let remaining_pct = msg.battery_remaining as f32;
    let raw = msg.voltages[0];
    let voltage_v = if raw == u16::MAX {
        session.last_battery_voltage_v.unwrap_or(0.0)
    } else {
        (raw as f32) / 1000.0
    };

    session.pending_sample.battery_remaining_pct = Some(remaining_pct);
    session.pending_sample.touch();

    let last = session.last_battery_remaining_pct;
    let changed = match last {
        None => true,
        Some(prev) => (prev - remaining_pct).abs() >= 1.0,
    };
    if !changed {
        return Vec::new();
    }
    session.last_battery_remaining_pct = Some(remaining_pct);
    session.last_battery_voltage_v = Some(voltage_v);
    vec![NormalizedEventKind::BatteryUpdated {
        remaining_pct,
        voltage_v,
    }]
}

/// Translate GPS_RAW_INT — at v0.1 we only refine the telemetry sample's
/// altitude if no other source has set it; no event emitted.
pub fn translate_gps_raw(
    msg: &GPS_RAW_INT_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    if session.pending_sample.altitude_m.is_none() && msg.alt != 0 {
        session.pending_sample.altitude_m = Some((msg.alt as f64) / 1000.0);
        session.pending_sample.touch();
    }
    Vec::new()
}

/// Translate MISSION_CURRENT into a `WaypointChanged` (only on actual change).
pub fn translate_mission_current(
    msg: &MISSION_CURRENT_DATA,
    session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let index = msg.seq as u32;
    if session.last_waypoint == Some(index) {
        return Vec::new();
    }
    session.last_waypoint = Some(index);
    vec![NormalizedEventKind::WaypointChanged { index }]
}

/// Translate STATUSTEXT into a `StatustextReceived`.
pub fn translate_statustext(
    msg: &STATUSTEXT_DATA,
    _session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let severity = severity_to_u8(msg.severity);
    let text = msg
        .text
        .to_str()
        .unwrap_or("")
        .trim_end_matches('\0')
        .to_string();
    if text.is_empty() {
        return Vec::new();
    }
    vec![NormalizedEventKind::StatustextReceived { severity, text }]
}

/// Translate COMMAND_ACK into a `CommandAccepted` or `CommandRejected`.
pub fn translate_command_ack(
    msg: &COMMAND_ACK_DATA,
    _session: &mut SessionState,
) -> Vec<NormalizedEventKind> {
    let command = format!("{:?}", msg.command);
    match msg.result {
        MavResult::MAV_RESULT_ACCEPTED => {
            vec![NormalizedEventKind::CommandAccepted { command }]
        }
        other => {
            let reason = format!("{other:?}");
            vec![NormalizedEventKind::CommandRejected { command, reason }]
        }
    }
}

/// Map a `MavSeverity` enum value to its underlying 0..=7 number.
fn severity_to_u8(severity: mavlink::dialects::common::MavSeverity) -> u8 {
    use mavlink::dialects::common::MavSeverity::*;
    match severity {
        MAV_SEVERITY_EMERGENCY => 0,
        MAV_SEVERITY_ALERT => 1,
        MAV_SEVERITY_CRITICAL => 2,
        MAV_SEVERITY_ERROR => 3,
        MAV_SEVERITY_WARNING => 4,
        MAV_SEVERITY_NOTICE => 5,
        MAV_SEVERITY_INFO => 6,
        MAV_SEVERITY_DEBUG => 7,
    }
}

/// Derive a human-ish mode string from a HEARTBEAT.
///
/// For ArduPilot Copter we map a handful of common custom-mode numbers to
/// their canonical strings; otherwise we fall back to `custom_mode=N`.
fn mode_string(msg: &HEARTBEAT_DATA) -> String {
    if !msg
        .base_mode
        .contains(MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED)
    {
        return format!("base_mode={}", msg.base_mode.bits());
    }
    if let Some(name) = ardupilot_copter_mode(msg.mavtype, msg.custom_mode) {
        return name.to_string();
    }
    format!("custom_mode={}", msg.custom_mode)
}

fn ardupilot_copter_mode(mavtype: MavType, custom_mode: u32) -> Option<&'static str> {
    match mavtype {
        MavType::MAV_TYPE_QUADROTOR
        | MavType::MAV_TYPE_HEXAROTOR
        | MavType::MAV_TYPE_OCTOROTOR
        | MavType::MAV_TYPE_TRICOPTER
        | MavType::MAV_TYPE_COAXIAL
        | MavType::MAV_TYPE_HELICOPTER => match custom_mode {
            0 => Some("STABILIZE"),
            1 => Some("ACRO"),
            2 => Some("ALT_HOLD"),
            3 => Some("AUTO"),
            4 => Some("GUIDED"),
            5 => Some("LOITER"),
            6 => Some("RTL"),
            7 => Some("CIRCLE"),
            9 => Some("LAND"),
            16 => Some("POSHOLD"),
            17 => Some("BRAKE"),
            18 => Some("THROW"),
            19 => Some("AVOID_ADSB"),
            20 => Some("GUIDED_NOGPS"),
            21 => Some("SMART_RTL"),
            22 => Some("FLOWHOLD"),
            23 => Some("FOLLOW"),
            24 => Some("ZIGZAG"),
            _ => None,
        },
        _ => None,
    }
}

/// Best-effort vehicle label from a HEARTBEAT.
pub fn vehicle_label(msg: &HEARTBEAT_DATA) -> String {
    use mavlink::dialects::common::MavAutopilot::*;
    let autopilot = match msg.autopilot {
        MAV_AUTOPILOT_ARDUPILOTMEGA => "ardupilot",
        MAV_AUTOPILOT_PX4 => "px4",
        MAV_AUTOPILOT_GENERIC => "generic",
        MAV_AUTOPILOT_INVALID => "invalid",
        _ => "unknown",
    };
    let vehicle = match msg.mavtype {
        MavType::MAV_TYPE_QUADROTOR => "quad",
        MavType::MAV_TYPE_HEXAROTOR => "hex",
        MavType::MAV_TYPE_OCTOROTOR => "octo",
        MavType::MAV_TYPE_HELICOPTER => "heli",
        MavType::MAV_TYPE_FIXED_WING => "plane",
        MavType::MAV_TYPE_GROUND_ROVER => "rover",
        MavType::MAV_TYPE_SUBMARINE => "sub",
        _ => "vehicle",
    };
    format!("{autopilot}_{vehicle}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mavlink::dialects::common::{
        GpsFixType, MavAutopilot, MavBatteryFunction, MavBatteryType, MavCmd, MavSeverity,
        MavState, MavType,
    };
    use mavlink::types::CharArray;

    fn empty_session() -> SessionState {
        SessionState::default()
    }

    fn make_heartbeat(armed: bool, custom_mode: u32) -> HEARTBEAT_DATA {
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

    #[test]
    fn first_heartbeat_emits_connected_and_arming_state() {
        let mut s = empty_session();
        let hb = make_heartbeat(false, 4); // GUIDED
        let events = translate_heartbeat(&hb, &mut s);

        assert_eq!(events[0], NormalizedEventKind::VehicleConnected);
        assert_eq!(events[1], NormalizedEventKind::HeartbeatReceived);
        assert!(matches!(
            events[2],
            NormalizedEventKind::ArmedChanged { armed: false }
        ));
        assert!(
            matches!(&events[3], NormalizedEventKind::ModeChanged { mode } if mode == "GUIDED")
        );
        assert_eq!(s.last_armed, Some(false));
        assert_eq!(s.last_mode.as_deref(), Some("GUIDED"));
    }

    #[test]
    fn second_heartbeat_with_same_state_emits_only_heartbeat() {
        let mut s = empty_session();
        let hb = make_heartbeat(true, 4);
        let _ = translate_heartbeat(&hb, &mut s);
        let events = translate_heartbeat(&hb, &mut s);
        assert_eq!(events, vec![NormalizedEventKind::HeartbeatReceived]);
    }

    #[test]
    fn arming_transition_emits_armed_changed() {
        let mut s = empty_session();
        let _ = translate_heartbeat(&make_heartbeat(false, 4), &mut s);
        let events = translate_heartbeat(&make_heartbeat(true, 4), &mut s);
        assert!(events.contains(&NormalizedEventKind::HeartbeatReceived));
        assert!(events.contains(&NormalizedEventKind::ArmedChanged { armed: true }));
    }

    #[test]
    fn mode_change_emits_mode_changed() {
        let mut s = empty_session();
        let _ = translate_heartbeat(&make_heartbeat(false, 4), &mut s);
        let events = translate_heartbeat(&make_heartbeat(false, 3), &mut s);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, NormalizedEventKind::ModeChanged { mode } if mode == "AUTO"))
        );
    }

    #[test]
    fn unknown_mode_falls_back_to_custom_mode_n() {
        let mut s = empty_session();
        let events = translate_heartbeat(&make_heartbeat(false, 999), &mut s);
        assert!(events.iter().any(
            |e| matches!(e, NormalizedEventKind::ModeChanged { mode } if mode == "custom_mode=999")
        ));
    }

    #[test]
    fn global_position_int_emits_position_and_altitude_and_updates_sample() {
        let mut s = empty_session();
        let msg = GLOBAL_POSITION_INT_DATA {
            time_boot_ms: 1000,
            lat: 473977000,     // 47.3977
            lon: 85456000,      // 8.5456
            alt: 500_000,       // 500m MSL
            relative_alt: 5500, // 5.5m above home
            vx: 0,
            vy: 0,
            vz: -120, // -1.2 m/s in MAVLink frame (= +1.2 m/s up)
            hdg: 0,
        };
        let events = translate_global_position(&msg, &mut s);
        assert!(
            matches!(events[0], NormalizedEventKind::PositionUpdated { lat, lon }
                        if (lat - 47.3977).abs() < 1e-6 && (lon - 8.5456).abs() < 1e-6)
        );
        assert!(
            matches!(events[1], NormalizedEventKind::AltitudeUpdated { altitude_m }
                        if (altitude_m - 5.5).abs() < 1e-6)
        );
        assert!(s.pending_sample.dirty);
        assert!((s.pending_sample.altitude_m.unwrap() - 5.5).abs() < 1e-6);
        assert!((s.pending_sample.vertical_speed_mps.unwrap() - 1.2).abs() < 1e-6);
    }

    #[test]
    fn vfr_hud_sets_vertical_speed_only() {
        let mut s = empty_session();
        let msg = VFR_HUD_DATA {
            airspeed: 0.0,
            groundspeed: 0.0,
            alt: 12.5,
            climb: 2.4,
            heading: 0,
            throttle: 50,
        };
        let events = translate_vfr_hud(&msg, &mut s);
        assert!(events.is_empty());
        assert!((s.pending_sample.altitude_m.unwrap() - 12.5).abs() < 1e-6);
        assert!((s.pending_sample.vertical_speed_mps.unwrap() - 2.4).abs() < 1e-6);
    }

    #[test]
    fn sys_status_emits_battery_updated_only_on_meaningful_change() {
        let mut s = empty_session();
        let msg = SYS_STATUS_DATA {
            voltage_battery: 12400, // 12.4V
            battery_remaining: 85,
            ..SYS_STATUS_DATA::DEFAULT
        };
        let evs = translate_sys_status(&msg, &mut s);
        assert_eq!(evs.len(), 1);
        // Second identical sample suppresses event.
        let evs2 = translate_sys_status(&msg, &mut s);
        assert!(evs2.is_empty());
    }

    #[test]
    fn sys_status_with_negative_remaining_is_ignored() {
        let mut s = empty_session();
        let msg = SYS_STATUS_DATA {
            voltage_battery: 12400,
            battery_remaining: -1,
            ..SYS_STATUS_DATA::DEFAULT
        };
        let evs = translate_sys_status(&msg, &mut s);
        assert!(evs.is_empty());
    }

    #[test]
    fn battery_status_emits_battery_updated() {
        let mut s = empty_session();
        let mut voltages = [u16::MAX; 10];
        voltages[0] = 12400;
        let msg = BATTERY_STATUS_DATA {
            current_consumed: 0,
            energy_consumed: 0,
            temperature: 0,
            voltages,
            current_battery: 0,
            id: 0,
            battery_function: MavBatteryFunction::DEFAULT,
            mavtype: MavBatteryType::DEFAULT,
            battery_remaining: 50,
        };
        let evs = translate_battery_status(&msg, &mut s);
        assert_eq!(evs.len(), 1);
        if let NormalizedEventKind::BatteryUpdated {
            remaining_pct,
            voltage_v,
        } = evs[0]
        {
            assert!((remaining_pct - 50.0).abs() < 1e-3);
            assert!((voltage_v - 12.4).abs() < 1e-3);
        } else {
            panic!("expected BatteryUpdated");
        }
    }

    #[test]
    fn mission_current_emits_waypoint_changed_only_on_change() {
        let mut s = empty_session();
        let evs1 = translate_mission_current(&MISSION_CURRENT_DATA { seq: 0 }, &mut s);
        assert_eq!(
            evs1,
            vec![NormalizedEventKind::WaypointChanged { index: 0 }]
        );
        let evs2 = translate_mission_current(&MISSION_CURRENT_DATA { seq: 0 }, &mut s);
        assert!(evs2.is_empty());
        let evs3 = translate_mission_current(&MISSION_CURRENT_DATA { seq: 3 }, &mut s);
        assert_eq!(
            evs3,
            vec![NormalizedEventKind::WaypointChanged { index: 3 }]
        );
    }

    #[test]
    fn statustext_emits_event_with_severity() {
        let mut s = empty_session();
        let msg = STATUSTEXT_DATA {
            severity: MavSeverity::MAV_SEVERITY_WARNING,
            text: CharArray::from("EKF altitude variance"),
        };
        let evs = translate_statustext(&msg, &mut s);
        assert_eq!(evs.len(), 1);
        if let NormalizedEventKind::StatustextReceived { severity, text } = &evs[0] {
            assert_eq!(*severity, 4);
            assert_eq!(text, "EKF altitude variance");
        } else {
            panic!("expected StatustextReceived");
        }
    }

    #[test]
    fn statustext_empty_text_is_ignored() {
        let mut s = empty_session();
        let msg = STATUSTEXT_DATA {
            severity: MavSeverity::MAV_SEVERITY_INFO,
            text: CharArray::new([0u8; 50]),
        };
        let evs = translate_statustext(&msg, &mut s);
        assert!(evs.is_empty());
    }

    #[test]
    fn command_ack_accepted_emits_command_accepted() {
        let mut s = empty_session();
        let msg = COMMAND_ACK_DATA {
            command: MavCmd::MAV_CMD_NAV_TAKEOFF,
            result: MavResult::MAV_RESULT_ACCEPTED,
        };
        let evs = translate_command_ack(&msg, &mut s);
        assert!(
            matches!(&evs[0], NormalizedEventKind::CommandAccepted { command }
                         if command.contains("NAV_TAKEOFF"))
        );
    }

    #[test]
    fn command_ack_denied_emits_command_rejected() {
        let mut s = empty_session();
        let msg = COMMAND_ACK_DATA {
            command: MavCmd::MAV_CMD_NAV_TAKEOFF,
            result: MavResult::MAV_RESULT_DENIED,
        };
        let evs = translate_command_ack(&msg, &mut s);
        assert!(
            matches!(&evs[0], NormalizedEventKind::CommandRejected { command, reason }
                         if command.contains("NAV_TAKEOFF") && reason.contains("DENIED"))
        );
    }

    #[test]
    fn gps_raw_int_fills_altitude_when_missing() {
        let mut s = empty_session();
        let msg = GPS_RAW_INT_DATA {
            time_usec: 0,
            lat: 0,
            lon: 0,
            alt: 7500, // 7.5m
            eph: 0,
            epv: 0,
            vel: 0,
            cog: 0,
            fix_type: GpsFixType::DEFAULT,
            satellites_visible: 12,
        };
        let _ = translate_gps_raw(&msg, &mut s);
        assert!((s.pending_sample.altitude_m.unwrap() - 7.5).abs() < 1e-6);
    }

    #[test]
    fn vehicle_label_quad_ardupilot() {
        let hb = make_heartbeat(false, 4);
        assert_eq!(vehicle_label(&hb), "ardupilot_quad");
    }
}
