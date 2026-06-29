# Issue 4 — MAVLink ingestion from SITL

## Summary

Add a read-only MAVLink UDP ingestion path so FlightTrace can record real mission runs from PX4 or ArduPilot SITL (and, in principle, real hardware). MAVLink messages are parsed and translated into the same normalized events and telemetry samples the demo uses, then persisted through the storage layer from issue 2. The UI from issue 3 shows the resulting runs side-by-side with demo runs.

This is the first issue where FlightTrace touches real autopilot data. It must remain strictly read-only.

## Product goal

After this issue, an engineer can:

1. Start PX4 or ArduPilot SITL exposing MAVLink on a UDP endpoint (e.g. `udp://127.0.0.1:14550`).
2. Start FlightTrace with `cargo run -p flighttrace-cli -- serve` and a new `--mavlink udp://127.0.0.1:14550` flag (or equivalent).
3. Watch a new mission run appear in the UI as soon as the heartbeat is received.
4. See live-ish telemetry samples and normalized events as SITL flies a mission.
5. If the heartbeat drops or telemetry stalls, see a `TelemetryGapDetected` event and (later, via issue 5 assertions) a corresponding failure.

## Technical scope

- New crate `flighttrace-mavlink` that owns the MAVLink UDP listener, message parsing, and translation into normalized events / telemetry samples.
- Selected MAVLink message support (see list below).
- A "session" concept that maps to one `MissionRun`. A session begins on the first heartbeat after the listener starts (or after a configurable idle gap) and ends when explicitly stopped or after a configurable inactivity timeout.
- Heartbeat tracking and telemetry-gap detection.
- Integration with `flighttrace-storage` for persistence.
- CLI flags to enable ingestion and pick the endpoint.
- README notes for PX4 and ArduPilot SITL.

## Files and modules to create or modify

```
crates/
  flighttrace-mavlink/
    Cargo.toml
    src/
      lib.rs
      config.rs                                 # MavlinkConfig (endpoint, dialect, idle timeout)
      listener.rs                               # UDP listener + parse loop (Tokio)
      session.rs                                # MavlinkSession -> one MissionRun
      translate.rs                              # MAVLink message -> NormalizedEventKind / TelemetrySample
      gap.rs                                    # heartbeat + telemetry gap detection
      error.rs                                  # MavlinkIngestError
  flighttrace-api/
    src/
      handlers/
        runs.rs                                 # (optional) GET /api/runs/:id/telemetry
  flighttrace-cli/
    src/main.rs                                 # add --mavlink flag to `serve`
docs/
  prd.md                                        # update if endpoints change
  issues/
    04-mavlink-ingestion.md                     # this file
README.md                                       # add SITL connection section
```

## Detailed implementation steps

1. **Pick the MAVLink crate.**
   - Use `mavlink` (the `rust-mavlink` crate on crates.io) unless a clearly better-maintained option exists at implementation time.
   - Use the `ardupilotmega` or `common` dialect feature; default to `common` for cross-stack compatibility.
   - Pin a known-good version and document the choice in the crate's `Cargo.toml` comment.

2. **`flighttrace-mavlink` crate.**
   - Dependencies: `mavlink`, `tokio` (`net`, `time`, `sync`, `macros`, `rt`), `flighttrace-core`, `flighttrace-storage`, `tracing`, `thiserror`, `serde`, `uuid`, `time` or `chrono`.
   - `MavlinkConfig` fields: `endpoint: String` (parsed from `udp://host:port`), `dialect: Dialect` (enum), `heartbeat_timeout: Duration` (default 3 s), `telemetry_gap_threshold: Duration` (default 2 s), `session_idle_timeout: Duration` (default 30 s).
   - `listener.rs`:
     - `pub async fn run(config: MavlinkConfig, storage: StorageHandles, shutdown: CancellationToken) -> Result<(), MavlinkIngestError>`
     - Binds a UDP socket (Tokio's `UdpSocket`). Reads datagrams, hands them to the `mavlink` parser.
     - Owns one `MavlinkSession` at a time. Creates a new session when the first heartbeat is observed after a fresh start or after `session_idle_timeout`.
   - `session.rs`:
     - On creation: inserts a new `MissionRun` with `source = Mavlink`, `status = Running`, `vehicle` inferred from `HEARTBEAT.autopilot` (e.g. `px4` / `ardupilot_copter`), `mission = "live"` placeholder, `started_at = now`.
     - On each parsed message: passes it to `translate.rs`; persists resulting normalized events and telemetry samples through the repos.
     - On gap or shutdown: closes the session, updates `ended_at`, and (for now) sets `status = Succeeded` unless a `MissionFailed` event was recorded — assertion-driven failure will come in issue 5.
   - `translate.rs` — initial message support:
     - `HEARTBEAT` → `HeartbeatReceived`; on first heartbeat also `VehicleConnected`; on autopilot mode change emit `ModeChanged`; on armed-state change emit `ArmedChanged`.
     - `GLOBAL_POSITION_INT` → `PositionUpdated` + a `TelemetrySample` carrying altitude (`relative_alt` in mm, convert to m) and vertical speed (`vz` cm/s, convert to m/s, sign-flipped to "up positive" if needed).
     - `ATTITUDE` or `VFR_HUD` → contribute to the current telemetry sample (e.g. fill `vertical_speed_mps` from `VFR_HUD.climb`).
     - `SYS_STATUS` / `BATTERY_STATUS` → `BatteryUpdated` + sample's `battery_remaining_pct`.
     - `GPS_RAW_INT` → no event by default; later we can emit `GpsUnhealthy` when fix type / satellites drop.
     - `MISSION_CURRENT` → `WaypointChanged { index }`.
     - `STATUSTEXT` → `StatustextReceived { severity, text }`.
     - `COMMAND_ACK` → `CommandAccepted` (result == `MAV_RESULT_ACCEPTED`) or `CommandRejected` (other) with the command name from `command`.
   - `gap.rs`:
     - Track last-seen heartbeat time; if `now - last_heartbeat > heartbeat_timeout`, emit `TelemetryGapDetected { gap_sec }` once per gap and log a `warn`.
     - Track last-seen telemetry sample time; same logic with `telemetry_gap_threshold`.

3. **Throttling and batching.**
   - Telemetry-sample inserts should be batched per ~250 ms window to avoid one SQLite write per UDP packet at 50 Hz.
   - Normalized events are written individually (they are low-rate).
   - Use a Tokio `mpsc` channel between parser and persister; persister flushes batches.

4. **CLI integration.**
   - Add `--mavlink <endpoint>` to the `serve` subcommand. When present, spawn the ingestion task alongside the Axum server. When absent, the server runs as before (demo-only).
   - Add `--mavlink-dialect <common|ardupilotmega|px4>` with default `common`.
   - On `Ctrl+C`, gracefully cancel the ingestion task and close the active session cleanly (update `ended_at`).

5. **API surface (minimal additions).**
   - Optional: add `GET /api/runs/:id/telemetry?since=<ts>` returning telemetry samples for the run, useful for the UI to refresh charts. If implemented, document the response shape in this issue's PR.
   - Optional: add `POST /api/sessions/stop` to manually end the active MAVLink session. Not required; the inactivity timeout is enough for v0.1.
   - Do not add any command-sending endpoints. Period.

6. **UI behavior.**
   - No new UI components are required by this issue, but verify that a live MAVLink-sourced run renders correctly on the run detail page from issue 3 once it has been refreshed.
   - If live updates are desired, the simplest acceptable v0.1 addition is a "Refresh" button on the run detail page. SSE/WebSocket is a stretch goal.

7. **README — SITL section.**
   - Add a "Connecting to PX4 / ArduPilot SITL" section with:
     - The exact commands to launch SITL with MAVLink on `udp://127.0.0.1:14550` for both PX4 and ArduPilot.
     - The exact FlightTrace command: `cargo run -p flighttrace-cli -- serve --mavlink udp://127.0.0.1:14550`.
     - A note that FlightTrace is **read-only** and does not send commands.

8. **Safety.**
   - The MAVLink listener must never write to the socket.
   - The crate's public API exposes no "send" function. If the `mavlink` crate provides senders, they must not be wired anywhere in FlightTrace.
   - Add a comment at the top of `listener.rs` and `lib.rs`: `// FlightTrace MAVLink ingestion is strictly read-only. Do not add send paths.`

## Acceptance criteria

- [ ] `flighttrace-mavlink` crate compiles, with documented dialect feature.
- [ ] `cargo run -p flighttrace-cli -- serve --mavlink udp://127.0.0.1:14550` starts the API and the MAVLink listener.
- [ ] Running ArduPilot SITL or PX4 SITL on the chosen UDP endpoint produces a new `MissionRun` with `source = mavlink` visible in `GET /api/runs` and in the UI.
- [ ] The run's normalized events include `HeartbeatReceived`, `ArmedChanged`, `ModeChanged`, and at least one telemetry-derived event over a typical SITL takeoff.
- [ ] Telemetry samples are persisted at a rate that does not overwhelm SQLite (batched writes).
- [ ] When the SITL stream is interrupted, a `TelemetryGapDetected` event is recorded within a few seconds.
- [ ] No FlightTrace code path can send MAVLink messages. (Verified by code review and a unit test that asserts the public API exposes no send function.)
- [ ] README "Connecting to PX4 / ArduPilot SITL" section is present and accurate.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass.

## Tests required

- Unit: `translate.rs` correctly maps each supported MAVLink message to the expected normalized event / sample. Use hand-constructed MAVLink frames or the crate's builders; no live network needed.
- Unit: heartbeat gap detection emits a `TelemetryGapDetected` exactly once per gap.
- Unit: read-only invariant — `flighttrace-mavlink::lib` does not re-export any send function and no code in the crate calls a send method on a MAVLink connection.
- Integration: in-memory channel test where a sequence of fake parsed messages drives a `MavlinkSession` and the resulting bundle in storage has the expected events and samples.
- Manual: at least one SITL session (PX4 or ArduPilot) reported in the PR description, with a screenshot or log snippet showing live events.

## Out of scope

- Sending any MAVLink command (arm, takeoff, mode, mission upload, parameter set, anything).
- Mission upload or mission download.
- Companion-computer agent.
- ROS 2 integration.
- TCP / serial MAVLink transports (UDP only in v0.1; the config struct should leave room to add them later).
- Multi-vehicle ingestion (one active session at a time).
- Per-message advanced derivation (e.g. EKF health analysis) beyond what is needed for the listed events.

## Notes for future issues

- **Issue 5** (assertions) will run YAML assertions against runs produced here. The set of normalized events emitted by `translate.rs` is the contract assertions evaluate against — keep it small and consistent.
- **Issue 5** will likely add `MissionFailed` events to MAVLink runs when assertions fail; session.rs should be ready to react to assertion outcomes and transition the run to `Failed`.
- **Issue 6** (docs) will expand the SITL section with troubleshooting (firewall, port conflicts, dialect mismatch).
- If multi-vehicle support is ever added, the session model will need to key on `system_id`; that is explicitly out of scope here.
- If live UI streaming is added, prefer SSE for simplicity unless bidirectional traffic is genuinely required.
