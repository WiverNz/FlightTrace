# Issue 5 — Assertion engine and failure reports

## Summary

Replace the hard-coded assertion results from issue 1 with a real **assertion engine** that parses mission expectations from YAML, evaluates them against normalized events and telemetry samples, and produces structured failure reports with cited evidence. Both demo runs and MAVLink-sourced runs use the same engine.

## Product goal

After this issue, an engineer can:

1. Write a small YAML file under `examples/missions/` (e.g. `simple_takeoff.yaml`) describing what the mission is expected to do.
2. Tag a mission run with that mission file (the demo already uses `mission: "simple_takeoff"`; MAVLink runs can be tagged via CLI flag or default).
3. After the run finishes (or as a manual re-evaluation pass), see assertion results in the UI: which steps passed, warned, or failed, with detail strings.
4. For failing runs, see a `FailureReport` whose evidence list cites the exact events and telemetry samples that triggered the failure, and whose `failure_reason` is derived from the kind of assertion that failed.

This is the issue that turns FlightTrace from "demo viewer" into a real mission debugger.

## Technical scope

- New crate `flighttrace-assertions` with a YAML parser, a small declarative assertion DSL, an evaluator, and a failure-report builder.
- Integration with `flighttrace-demo` so the demo's assertion results come from running the engine against the demo bundle (no more hard-coding).
- Integration with `flighttrace-mavlink` so live runs are evaluated when they end (and optionally re-evaluated on demand).
- An API endpoint to re-evaluate a stored run against a mission file.
- Tests for the takeoff-timeout scenario and at least one other scenario (telemetry gap).

## Files and modules to create or modify

```
crates/
  flighttrace-assertions/
    Cargo.toml
    src/
      lib.rs
      schema.rs                                 # serde structs for the YAML format
      parser.rs                                 # YAML -> MissionSpec
      engine.rs                                 # evaluate(MissionSpec, RunBundle) -> Vec<AssertionResult>
      checks/
        mod.rs
        heartbeat.rs                            # heartbeat_within_sec
        armed.rs                                # armed
        altitude.rs                             # altitude_at_least_m within time
        mode.rs                                 # mode_not_changes
        gap.rs                                  # no_telemetry_gap_longer_than_sec
      failure.rs                                # build FailureReport from failing assertions
      error.rs                                  # AssertionError
  flighttrace-demo/
    src/takeoff_timeout.rs                      # drop hard-coded assertion results; call the engine
  flighttrace-mavlink/
    src/session.rs                              # on session end, run the engine and persist results
  flighttrace-api/
    src/handlers/runs.rs                        # POST /api/runs/:id/reevaluate
  examples/
    missions/
      simple_takeoff.yaml
docs/
  prd.md                                        # add assertion DSL reference if it changed
```

## Detailed implementation steps

1. **Mission YAML schema.**
   - Top-level fields: `mission: <string>`, `vehicle: <string>`, `steps: [ ... ]`.
   - Each step has `name: <string>` and `expect: { ... }`. The `expect` map holds one or more checks. Optional global `within_sec` on a step applies to all time-bounded checks in that step unless overridden.
   - Supported checks for v0.1:
     - `heartbeat_within_sec: <number>`
     - `armed: <bool>`
     - `altitude_at_least_m: <number>` (paired with `within_sec`)
     - `mode_not_changes: <bool>`
     - `no_telemetry_gap_longer_than_sec: <number>`
   - Example (used as `examples/missions/simple_takeoff.yaml`):
     ```yaml
     mission: simple_takeoff
     vehicle: ardupilot_copter
     steps:
       - name: connect
         expect:
           heartbeat_within_sec: 5
       - name: arm
         expect:
           armed: true
           within_sec: 5
       - name: takeoff
         expect:
           altitude_at_least_m: 10
           within_sec: 30
           mode_not_changes: true
           no_telemetry_gap_longer_than_sec: 2
     ```

2. **`flighttrace-assertions` crate.**
   - Dependencies: `serde`, `serde_yaml`, `flighttrace-core`, `thiserror`, `tracing`, `time` or `chrono`, `uuid`.
   - `schema.rs` defines `MissionSpec`, `StepSpec`, and `ExpectSpec` (a struct with optional fields for each supported check). Reject unknown keys at parse time.
   - `parser.rs` exposes `pub fn parse_mission_yaml(s: &str) -> Result<MissionSpec, AssertionError>` and `pub fn load_mission_file(path: &Path) -> Result<MissionSpec, AssertionError>`.
   - `engine.rs` exposes:
     - `pub struct EvaluationInput<'a> { pub run: &'a MissionRun, pub events: &'a [NormalizedEvent], pub samples: &'a [TelemetrySample] }`
     - `pub fn evaluate(spec: &MissionSpec, input: &EvaluationInput) -> EvaluationOutput`
     - `pub struct EvaluationOutput { pub results: Vec<AssertionResult>, pub failure_report: Option<FailureReport> }`
   - Each check in `checks/` is a pure function that takes the step name, the spec, and the input, and returns one `AssertionResult` plus a list of `Evidence` items it used.

3. **Failure-report derivation.**
   - When any assertion in a step fails, the engine produces a `FailureReport` with:
     - `reason` derived by mapping the failing check to a `FailureReason`:
       - `heartbeat_within_sec` failure → `NoHeartbeat`
       - `armed` failure → `CommandTimeout` (no arming) or `Unknown` if there is no command evidence
       - `altitude_at_least_m` failure → `TakeoffTimeout`
       - `mode_not_changes` failure → `UnexpectedModeChange`
       - `no_telemetry_gap_longer_than_sec` failure → `TelemetryGap`
     - `summary` = `"Mission failed: <REASON>"`
     - `evidence` = union of evidence cited by failing checks, deduplicated
     - `likely_cause` = a short paragraph per failure reason (table in `failure.rs`)
   - If multiple assertions fail, pick the first failing step's reason for the report's `reason` (deterministic ordering by step index, then by check name).

4. **Demo integration.**
   - Move the `simple_takeoff.yaml` mission file into `examples/missions/` and update `flighttrace-demo` to:
     - Build the same deterministic events and samples as before.
     - Parse `examples/missions/simple_takeoff.yaml` (embed it via `include_str!`).
     - Call `flighttrace_assertions::evaluate(...)` to produce the assertion results and failure report.
   - The visible output of the demo must be the same as before this issue — same `TAKEOFF_TIMEOUT` reason, same kind of evidence — but now produced by the real engine.

5. **MAVLink integration.**
   - When a `MavlinkSession` ends (either by inactivity or shutdown), it:
     - Loads the active mission spec (configurable via a new `--mission <path>` CLI flag on `serve`; default is no spec, in which case evaluation is skipped).
     - Loads the run's persisted events and samples.
     - Calls `flighttrace_assertions::evaluate(...)`.
     - Persists the resulting `AssertionResult`s and (if any) the `FailureReport`.
     - Sets `MissionRun.status` to `Failed` and `failure_reason` accordingly when a report is produced; otherwise `Succeeded`.

6. **Re-evaluation API.**
   - `POST /api/runs/:id/reevaluate` with optional JSON body `{ "missionPath": "examples/missions/simple_takeoff.yaml" }`. If the body is omitted, fall back to the run's tagged mission file.
   - The handler loads the bundle, runs the engine, writes the new `AssertionResult`s (replacing prior results for that run), and writes / overwrites the `FailureReport`.
   - Returns the updated assertion results and failure report.

7. **Tests.**
   - Unit: each check in `checks/` is covered with both a passing and a failing input.
   - Unit: parser rejects unknown keys, missing required fields, and non-numeric `within_sec`.
   - Integration: feeding the issue-1 demo bundle through the engine produces the same `TAKEOFF_TIMEOUT` failure report shape that was hard-coded before.
   - Integration: a synthetic bundle with a 5-second telemetry gap produces a `TelemetryGap` failure report with the gap event as evidence.
   - Integration: `POST /api/runs/:id/reevaluate` against the persisted demo run returns a `failed` outcome and the same reason.

## Acceptance criteria

- [ ] `flighttrace-assertions` crate compiles and exposes the documented public API.
- [ ] `examples/missions/simple_takeoff.yaml` exists and matches the schema.
- [ ] The demo run's assertion results and failure report are produced by `flighttrace-assertions`, not hard-coded.
- [ ] The demo run still ends in `failed` with reason `TAKEOFF_TIMEOUT` and an evidence list and likely-cause paragraph consistent with the PRD.
- [ ] At least the five v0.1 checks are implemented (`heartbeat_within_sec`, `armed`, `altitude_at_least_m`, `mode_not_changes`, `no_telemetry_gap_longer_than_sec`).
- [ ] MAVLink sessions, on close, run the engine when a mission spec is configured and persist results.
- [ ] `POST /api/runs/:id/reevaluate` returns updated assertion results and failure report.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass.

## Tests required

- Per-check unit tests (pass and fail).
- YAML parser tests for valid and invalid documents.
- End-to-end test: demo bundle → engine → `TAKEOFF_TIMEOUT` failure report with non-empty evidence.
- End-to-end test: synthetic telemetry-gap bundle → `TelemetryGap` failure report.
- API test: `POST /api/runs/:id/reevaluate` round-trip.

## Out of scope

- No scripting language (no Rhai, no Lua, no JavaScript). Declarative YAML only.
- No arbitrary user code execution.
- No machine-learning or LLM-based diagnosis.
- No physics simulation / environmental modeling.
- No conditional logic, loops, or branching in the YAML DSL. Each step is a flat list of checks.
- No assertion authoring UI.
- No streaming evaluation during a live run (evaluation runs on session close or via the re-evaluate endpoint).

## Notes for future issues

- **Issue 6** (docs) will document the assertion DSL in the README with the YAML example.
- Future versions may add `parameter` checks (e.g. `param_equals`), `waypoint_reached_within_sec`, `gps_fix_at_least`, `battery_above_pct`. The `ExpectSpec` shape and the `checks/` directory structure should make adding these mechanical.
- Future versions may stream evaluation incrementally; design `engine.rs` to keep `evaluate` pure and to permit a future incremental evaluator that consumes events as they arrive.
- Failure-report `likely_cause` strings live in code today; a future issue may move them to a localized resource file, but not in v0.1.
