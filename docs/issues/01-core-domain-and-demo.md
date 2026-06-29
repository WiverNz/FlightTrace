# Issue 1 — Core domain model and demo run generator

## Summary

Set up the FlightTrace Rust workspace and implement the **core domain model** (mission run, normalized events, telemetry samples, assertion results, failure report) together with a **deterministic failed demo run generator**. No database, no HTTP API, no UI, no MAVLink yet. The output of this issue is a library and a unit-tested in-memory demo run that downstream issues will persist and display.

## Product goal

After this issue, an engineer can call `flighttrace_demo::generate_failed_takeoff_run()` from a test or a small example binary and get back a fully populated `MissionRun` value that:

- has status `Failed`
- has failure reason `TakeoffTimeout`
- contains a coherent mission timeline (Disconnected → Connected → Armed → TakeoffRequested → Climbing → Failed)
- contains normalized events including a `StatustextReceived("EKF altitude variance")`
- contains telemetry samples with altitude rising from 0 m to 2.4 m and stalling there
- contains at least one failing assertion result for "takeoff reaches 10 m"
- contains a failure report with structured evidence and a likely-cause paragraph

This is the deterministic seed for the whole product. Issues 2 and 3 will persist and render it; issue 5 will generalize the assertion logic that issue 1 hard-codes.

## Technical scope

- Convert the current single-crate `Cargo.toml` into a Cargo **workspace**.
- Create the `flighttrace-core` crate with the domain model.
- Create the `flighttrace-demo` crate that depends on `flighttrace-core` and produces the deterministic failed run.
- Create the `flighttrace-cli` crate as a thin binary that, for now, prints a serialized demo run to stdout (this gives us a manual smoke test before the API and UI exist).
- Unit tests covering the demo run shape and key invariants.

This issue introduces **no** I/O beyond stdout, **no** HTTP, **no** SQL, **no** MAVLink.

## Files and modules to create or modify

```
Cargo.toml                                       # convert to workspace
src/main.rs                                      # delete (moved into flighttrace-cli)
crates/
  flighttrace-core/
    Cargo.toml
    src/
      lib.rs
      ids.rs                                     # MissionRunId, EventId, etc.
      time.rs                                    # timestamp helpers
      mission.rs                                 # MissionRun, MissionPhase, MissionStatus
      events.rs                                  # NormalizedEvent enum + payload structs
      telemetry.rs                               # TelemetrySample
      assertions.rs                              # AssertionResult, AssertionOutcome
      failure.rs                                 # FailureReport, Evidence, FailureReason
      error.rs                                   # FlightTraceError (thiserror)
  flighttrace-demo/
    Cargo.toml
    src/
      lib.rs                                     # pub fn generate_failed_takeoff_run()
      takeoff_timeout.rs                         # the deterministic scenario
  flighttrace-cli/
    Cargo.toml
    src/main.rs                                  # `flighttrace demo` subcommand prints JSON
```

## Detailed implementation steps

1. **Workspace conversion.**
   - Replace the root `Cargo.toml` with a workspace manifest declaring `crates/*` as members.
   - Delete `src/main.rs` from the root; the binary moves into `flighttrace-cli`.
   - Set `resolver = "2"`. Keep `edition = "2024"` consistent across crates if the toolchain supports it; otherwise pin to `2021`.

2. **`flighttrace-core` crate.**
   - Add dependencies: `serde`, `serde_json`, `uuid` (with `v4` and `serde` features), `time` or `chrono` (pick one; prefer `time` with `serde` + `formatting` features), `thiserror`.
   - In `ids.rs`, define newtype wrappers (`MissionRunId`, `EventId`, `AssertionResultId`, `FailureReportId`) around `uuid::Uuid`, each `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`.
   - In `time.rs`, define `pub type Timestamp = OffsetDateTime;` (or `DateTime<Utc>`), plus helpers for RFC3339 serialization.
   - In `mission.rs`:
     - `enum MissionPhase { Disconnected, Connected, Arming, Armed, TakeoffRequested, Climbing, TargetAltitudeReached, NavigatingToWaypoint, ReturningHome, Landing, Landed, Failed }`
     - `enum MissionStatus { Running, Succeeded, Failed }`
     - `enum MissionSource { Demo, Mavlink }`
     - `struct MissionRun { id, vehicle: String, mission: String, source: MissionSource, status: MissionStatus, failure_reason: Option<FailureReason>, started_at: Timestamp, ended_at: Option<Timestamp>, phases: Vec<PhaseTransition> }`
     - `struct PhaseTransition { at: Timestamp, phase: MissionPhase }`
   - In `events.rs`:
     - `struct NormalizedEvent { id: EventId, run_id: MissionRunId, at: Timestamp, kind: NormalizedEventKind }`
     - `enum NormalizedEventKind` with variants matching the PRD list: `VehicleConnected`, `HeartbeatReceived`, `ArmedChanged { armed: bool }`, `ModeChanged { mode: String }`, `CommandAccepted { command: String }`, `CommandRejected { command: String, reason: String }`, `PositionUpdated { lat: f64, lon: f64 }`, `AltitudeUpdated { altitude_m: f64 }`, `BatteryUpdated { remaining_pct: f32, voltage_v: f32 }`, `WaypointChanged { index: u32 }`, `TelemetryGapDetected { gap_sec: f64 }`, `StatustextReceived { severity: u8, text: String }`, `AssertionPassed { assertion_name: String }`, `AssertionFailed { assertion_name: String, detail: String }`, `MissionFailed { reason: FailureReason }`.
     - Derive `Serialize`, `Deserialize`, `Clone`, `Debug`.
   - In `telemetry.rs`:
     - `struct TelemetrySample { run_id: MissionRunId, at: Timestamp, altitude_m: f64, vertical_speed_mps: f64, mode: String, armed: bool, battery_remaining_pct: f32 }`
     - Keep it intentionally narrow for v0.1.
   - In `assertions.rs`:
     - `enum AssertionOutcome { Pass, Warn, Fail }`
     - `struct AssertionResult { id: AssertionResultId, run_id: MissionRunId, name: String, outcome: AssertionOutcome, detail: String, evaluated_at: Timestamp }`
   - In `failure.rs`:
     - `enum FailureReason { NoHeartbeat, CommandTimeout, TakeoffTimeout, WaypointNotReached, UnexpectedModeChange, TelemetryGap, LowBattery, GpsUnhealthy, Unknown }`
     - `enum Evidence { Event(EventId), Sample { at: Timestamp, field: String, value: String }, Statustext { at: Timestamp, text: String }, Note(String) }`
     - `struct FailureReport { id: FailureReportId, run_id: MissionRunId, reason: FailureReason, summary: String, evidence: Vec<Evidence>, likely_cause: String, generated_at: Timestamp }`
   - In `error.rs`, define `enum FlightTraceError` via `thiserror::Error` with variants for parse, validation, and "not found".

3. **`flighttrace-demo` crate.**
   - Depends on `flighttrace-core`.
   - Expose `pub fn generate_failed_takeoff_run() -> DemoRun` (or `MissionRunBundle`) where `DemoRun` is a struct bundling the `MissionRun`, `Vec<NormalizedEvent>`, `Vec<TelemetrySample>`, `Vec<AssertionResult>`, and `FailureReport`.
   - The generator must be **deterministic**:
     - Use a fixed `started_at` timestamp (e.g. `2025-01-01T00:00:00Z`).
     - Use a fixed UUID seed strategy (e.g. derive UUIDs from a counter, or hard-code a known set of UUIDs).
     - Use fixed time offsets for each event (in milliseconds from `started_at`).
   - The timeline encoded by the generator:
     - t+0.0 s — `VehicleConnected`, phase → `Connected`
     - t+0.5 s — `HeartbeatReceived`
     - t+1.0 s — `ModeChanged { mode: "GUIDED" }`
     - t+2.0 s — `ArmedChanged { armed: true }`, phase → `Armed`
     - t+2.5 s — `CommandAccepted { command: "MAV_CMD_NAV_TAKEOFF" }`, phase → `TakeoffRequested`
     - t+3.0 s..t+10.0 s — altitude samples rising from 0 m to ~2.4 m, then stalling; phase → `Climbing`
     - t+7.5 s — `StatustextReceived { severity: 4, text: "EKF altitude variance" }`
     - t+12.0 s — `AssertionFailed { assertion_name: "takeoff_reaches_10m", detail: "max_altitude=2.4m target=10.0m" }`
     - t+12.5 s — `MissionFailed { reason: TakeoffTimeout }`, phase → `Failed`
   - Produce one telemetry sample per ~500 ms in the climbing window, with realistic-looking but synthetic values for altitude, vertical speed, mode (`GUIDED`), armed (`true`), battery (`~85%`).
   - Produce assertion results: at least one `Pass` (e.g. `vehicle_connected`), at least one `Fail` (`takeoff_reaches_10m`).
   - Produce a `FailureReport` whose `summary` is `"Mission failed: TAKEOFF_TIMEOUT"`, whose `evidence` includes a `Note` line, sample references, and the statustext, and whose `likely_cause` matches the PRD's example paragraph.

4. **`flighttrace-cli` crate.**
   - Depends on `flighttrace-core`, `flighttrace-demo`, `serde_json`, `clap` (with `derive`).
   - One subcommand: `flighttrace demo`. It calls `generate_failed_takeoff_run()` and prints the bundle as pretty-printed JSON to stdout.
   - Wire `tracing-subscriber` for log output to stderr at `info` level.

5. **Unit tests.**
   - In `flighttrace-demo`, test that:
     - `generate_failed_takeoff_run()` is deterministic across two calls (same JSON output).
     - The returned run has `status == Failed` and `failure_reason == Some(TakeoffTimeout)`.
     - The event list contains at least one `MissionFailed` event and at least one `StatustextReceived` whose text contains `"EKF"`.
     - The maximum altitude in telemetry samples is between `2.0` and `3.0` m.
     - At least one assertion result has `outcome == Fail` and `name == "takeoff_reaches_10m"`.
     - The failure report's `reason` matches the run's `failure_reason`.
   - In `flighttrace-core`, test that all event/enum variants round-trip through `serde_json` without loss.

6. **Manual smoke test.**
   - `cargo run -p flighttrace-cli -- demo | head -n 40` prints the start of a coherent JSON bundle.

## Acceptance criteria

- [x] Root `Cargo.toml` is a workspace declaring `crates/flighttrace-core`, `crates/flighttrace-demo`, and `crates/flighttrace-cli` as members.
- [x] `flighttrace-core` defines `MissionRun`, `MissionPhase`, `MissionStatus`, `MissionSource`, `NormalizedEvent`, `NormalizedEventKind`, `TelemetrySample`, `AssertionResult`, `AssertionOutcome`, `FailureReport`, `FailureReason`, and `Evidence`, all with `serde` support.
- [x] `flighttrace-demo` exposes a deterministic `generate_failed_takeoff_run()` that returns a coherent failed-takeoff bundle matching the PRD scenario.
- [x] The bundle's failure report reproduces the PRD's example evidence and likely-cause paragraph.
- [x] `flighttrace-cli demo` prints the bundle as pretty JSON to stdout.
- [x] `cargo build --workspace` succeeds.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `cargo test --workspace` passes.

## Tests required

- Unit: serde round-trip for every `NormalizedEventKind` variant.
- Unit: determinism of `generate_failed_takeoff_run` (two calls produce equal JSON).
- Unit: structural invariants of the demo bundle (failed status, takeoff timeout reason, max altitude, failing assertion present).
- Unit: `FailureReport.evidence` is non-empty and references at least one statustext.

## Out of scope

- No SQLite, no sqlx, no migrations.
- No HTTP server, no Axum, no REST.
- No frontend.
- No MAVLink, no UDP, no SITL.
- No YAML assertion parser (issue 5 will introduce that). For now the demo can hard-code its assertion results.
- No command sending of any kind.
- No multi-run management beyond a single demo bundle in memory.

## Notes for future issues

- **Issue 2** will introduce `flighttrace-storage` and persist `MissionRun`, events, telemetry samples, assertion results, and failure reports. The schema should mirror these structs closely; keep the domain model SQL-friendly (no deeply nested non-serializable fields).
- **Issue 2** will also wire a `POST /api/demo/run` endpoint that calls `generate_failed_takeoff_run()`, persists the bundle, and returns the new run id. The generator must accept (or be wrappable to accept) a "now" timestamp so the persisted `started_at` can be the real current time, while keeping the *relative* timeline deterministic.
- **Issue 3** will render the bundle in the UI. The JSON shape produced here is effectively the API contract; treat field names as stable.
- **Issue 5** will replace the hard-coded assertion results in the demo with assertions evaluated by `flighttrace-assertions` from YAML. The demo bundle's assertion-result shape must remain compatible.

## Implementation notes

Issue 1 was implemented as planned. The full file inventory created:

```
Cargo.toml                                              # workspace manifest
crates/flighttrace-core/
  Cargo.toml
  src/
    lib.rs                                              # module wiring + re-exports
    time.rs                                             # Timestamp newtype, RFC3339 serde
    ids.rs                                              # MissionRunId / EventId / AssertionResultId / FailureReportId
    mission.rs                                          # MissionRun, MissionPhase, MissionStatus, MissionSource, PhaseTransition
    events.rs                                           # NormalizedEvent + NormalizedEventKind (internally tagged)
    telemetry.rs                                        # TelemetrySample
    assertions.rs                                       # AssertionResult, AssertionOutcome
    failure.rs                                          # FailureReason, Evidence, FailureReport
    error.rs                                            # FlightTraceError (thiserror)
crates/flighttrace-demo/
  Cargo.toml
  src/
    lib.rs                                              # DemoRun bundle + generate_failed_takeoff_run()
    takeoff_timeout.rs                                  # deterministic TAKEOFF_TIMEOUT scenario
crates/flighttrace-cli/
  Cargo.toml
  src/
    main.rs                                             # `flighttrace demo` subcommand
```

Quality gates run after implementation:

- `cargo fmt --all -- --check` — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo test --workspace` — 24 tests pass (13 in `flighttrace-core`, 11 in `flighttrace-demo`)
- `cargo run -p flighttrace-cli -- demo | head -n 40` — prints a coherent JSON bundle starting with the failed `MissionRun`, the five mission timeline phase transitions (`Connected`, `Armed`, `TakeoffRequested`, `Climbing`, `Failed`), and the first normalized events

Small, intentional deviations from the literal issue text:

- **`Timestamp` was implemented as a newtype** (`pub struct Timestamp(pub OffsetDateTime)`) with hand-written `Serialize`/`Deserialize` that emit RFC 3339 strings, rather than `pub type Timestamp = OffsetDateTime;`. This avoids per-field `#[serde(with = "...")]` annotations everywhere and gives a single place to control timestamp wire format. The issue allowed either `OffsetDateTime` or `DateTime<Utc>`; `time` was chosen.
- **`NormalizedEventKind` uses internally-tagged serde (`#[serde(tag = "kind")]`)** so the JSON for an event's `kind` field reads as e.g. `{"kind":"ArmedChanged","armed":true}` instead of the externally-tagged default `{"ArmedChanged":{"armed":true}}`. This is much friendlier for the future API and UI consumers; `Evidence` keeps the externally-tagged default because it has tuple variants.
- **Assertion result names were spelled `heartbeat_within_5s` and `armed_within_5s`** for the passing assertions (the issue named only the failing `takeoff_reaches_10m`). Names are stable strings issue 5's YAML engine can later reproduce.
- **Field names use Rust `snake_case`** in core (`failure_reason`, `run_id`, `vertical_speed_mps`, `battery_remaining_pct`). Per `docs/context.md` §11, the `camelCase` boundary is the HTTP API DTO layer that issue 2 will introduce; core domain types do not need to look like the wire format.
- **`flighttrace-cli` always initializes `tracing-subscriber` to stderr** (filter from `RUST_LOG`, defaults to `info`), regardless of subcommand. This keeps stdout a clean JSON pipe for `flighttrace demo`.

No scope crept in from issues 2–6: there is no SQLite, no HTTP, no MAVLink, no YAML assertion parser, and no command-sending code path.
