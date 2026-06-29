# FlightTrace — Product Requirements Document

Version: v0.1 (MVP)
Status: Pre-implementation
Owner: FlightTrace core team

## 1. One-line pitch

FlightTrace is a Rust-based, developer-first **mission debugger** for PX4 and ArduPilot autonomous missions. It records MAVLink telemetry, reconstructs the mission timeline, evaluates assertions, and **explains why a mission failed** with concrete evidence.

## 2. Problem

PX4 and ArduPilot developers, robotics engineers, and autonomy teams routinely run autonomous missions in SITL and on real hardware. When a mission misbehaves — the vehicle never reaches target altitude, switches mode unexpectedly, drops telemetry, or fails to arm — the debugging loop today is painful:

- Telemetry tools (QGroundControl, MAVProxy, AirData, Flight Review) show streams and plots, not failure causes.
- Logs are large, low-level, and disconnected from mission intent.
- Engineers re-run the mission and squint at MAVLink traffic to guess what went wrong.
- There is no shared, structured record of "this run failed, and here is the evidence."

The result: long debug cycles, tribal knowledge, and bugs that resurface across teams.

## 3. Product positioning

FlightTrace is **not**:

- a telemetry dashboard
- an AirData / Flight Review clone
- a fleet management or ops platform
- a ground control station
- a flight log viewer

FlightTrace **is**:

- a mission debugger
- a structured failure analysis tool
- a way to make mission failures explainable, reproducible, and reviewable

The core value proposition is one sentence:

> **Understand why autonomous missions fail.**

## 4. Target users

Primary:

- PX4 developers
- ArduPilot developers
- Robotics engineers writing autonomy / mission logic
- Developers testing autonomy behavior in SITL
- Teams debugging recurring autonomous mission failures

Secondary (later, not v0.1):

- QA engineers running regression flights
- Drone service companies validating mission templates
- Educators teaching autonomous flight stacks

Users are technical. They are comfortable with a terminal, YAML configs, and reading structured output. The product can be opinionated and dense; it should not be dumbed down for a non-developer audience.

## 5. Core user workflow (v0.1)

1. The developer starts FlightTrace locally (one backend, one frontend).
2. The developer opens the web UI in a browser.
3. The developer clicks **"Create demo failed run"**.
4. The backend generates a deterministic failed mission run end-to-end, with no need for PX4 or ArduPilot.
5. The new run appears in the runs list with status `failed` and reason `TAKEOFF_TIMEOUT`.
6. The developer opens the run detail page and sees:
   - mission status and failure reason
   - mission timeline (phase transitions)
   - telemetry samples (altitude, mode, armed state, battery)
   - assertion results (which passed, which failed)
   - evidence cards explaining the failure
   - raw normalized event log
7. The developer can re-run the demo and produces the same deterministic failed run.
8. In later issues, the developer can replace the demo with real MAVLink ingestion from PX4 or ArduPilot SITL.

This loop is the whole product in v0.1. It must feel like a serious debugging tool, not a dashboard.

## 6. Product vocabulary

These terms are used consistently in code, docs, UI, and conversation:

- **FlightTrace** — the product.
- **mission run** — one execution of a mission. Persistent. Has a status, a start time, an end time, events, telemetry, assertions, and optionally a failure report.
- **mission timeline** — the ordered sequence of mission phases for a run.
- **normalized event** — a structured, vehicle-agnostic event derived from MAVLink (or the demo generator). Examples: `VehicleConnected`, `ArmedChanged`, `AltitudeUpdated`, `MissionFailed`.
- **telemetry sample** — a time-stamped numeric snapshot (e.g. altitude, vertical speed, battery, position).
- **assertion** — a declarative expectation about a mission, evaluated against events and telemetry samples. Produces `pass`, `warn`, or `fail`.
- **failure report** — a structured explanation of why a mission run failed, including reason, evidence, and likely cause.
- **evidence** — concrete facts (events, samples, statustexts) cited inside a failure report.
- **SITL** — Software In The Loop simulator for PX4 / ArduPilot.
- **MAVLink** — the binary protocol PX4/ArduPilot use to communicate with ground stations and companion computers.
- **PX4** / **ArduPilot** — the two flight stacks FlightTrace targets.

## 7. Domain model (v0.1)

### Mission phases

```
Disconnected
Connected
Arming
Armed
TakeoffRequested
Climbing
TargetAltitudeReached
NavigatingToWaypoint
ReturningHome
Landing
Landed
Failed
```

### Normalized events

```
VehicleConnected
HeartbeatReceived
ArmedChanged
ModeChanged
CommandAccepted
CommandRejected
PositionUpdated
AltitudeUpdated
BatteryUpdated
WaypointChanged
TelemetryGapDetected
StatustextReceived
AssertionPassed
AssertionFailed
MissionFailed
```

### Failure reasons

```
NoHeartbeat
CommandTimeout
TakeoffTimeout
WaypointNotReached
UnexpectedModeChange
TelemetryGap
LowBattery
GpsUnhealthy
Unknown
```

### Mission run

A mission run has:

- `id` (UUID)
- `started_at` / `ended_at` timestamps
- `vehicle` label (e.g. `ardupilot_copter`, `px4_quad`, `demo`)
- `mission` label (e.g. `simple_takeoff`)
- `status` (`running`, `succeeded`, `failed`)
- `failure_reason` (nullable, one of the failure reasons above)
- `source` (`demo`, `mavlink`)
- references to its events, telemetry samples, assertion results, and failure report

### Failure report

A failure report has:

- mission run id
- failure reason (e.g. `TAKEOFF_TIMEOUT`)
- a short human summary line
- a list of **evidence** items (each is a typed fact: event reference, telemetry sample reference, or statustext)
- a **likely cause** paragraph

### Example failure report (the v0.1 demo target)

```
Mission failed: TAKEOFF_TIMEOUT

Evidence:
- Takeoff command was accepted.
- Vehicle armed successfully.
- Target altitude was 10.0 m.
- Maximum reached altitude was only 2.4 m.
- Vertical speed dropped near zero.
- Last known mode: GUIDED.
- Last STATUSTEXT: "EKF altitude variance".

Likely cause:
The vehicle stopped climbing before reaching target altitude.
Check estimator health, takeoff parameters, or mission timeout threshold.
```

## 8. Architecture (v0.1)

Local-first, single-machine, single-user. No cloud, no auth, no multi-tenancy.

```
+-----------------------------+        +--------------------------+
|  React + TypeScript + Vite  | <----> |  Axum HTTP API (Rust)    |
|  (web/)                     |  REST  |  flighttrace-api         |
|                             |  +SSE  |                          |
+-----------------------------+        +--------------------------+
                                                   |
                                                   v
                                       +--------------------------+
                                       | flighttrace-core         |
                                       | flighttrace-demo         |
                                       | flighttrace-assertions   |
                                       | flighttrace-storage      |
                                       | flighttrace-mavlink      |
                                       +--------------------------+
                                                   |
                                                   v
                                       +--------------------------+
                                       |  SQLite (sqlx)           |
                                       |  ./data/flighttrace.db   |
                                       +--------------------------+
                                                   ^
                                                   |
                                       +--------------------------+
                                       | PX4 / ArduPilot SITL     |
                                       | via MAVLink UDP          |
                                       | (issue 4)                |
                                       +--------------------------+
```

### Rust workspace layout

```
flighttrace/
  Cargo.toml                      # workspace
  crates/
    flighttrace-core/             # domain types, ids, time, errors
    flighttrace-demo/             # deterministic demo run generator
    flighttrace-storage/          # SQLite + sqlx, migrations, repos
    flighttrace-api/              # Axum server, REST + SSE
    flighttrace-mavlink/          # MAVLink UDP ingestion (issue 4)
    flighttrace-assertions/       # YAML assertions + failure reports
    flighttrace-cli/              # `flighttrace` binary entrypoint
  web/                            # React + TS + Vite UI
  examples/
    missions/simple_takeoff.yaml
  demo-runs/
  docs/
  README.md
  docker-compose.yml              # optional, later
```

### Tech stack

- Rust (stable), Tokio async runtime
- Axum for HTTP + SSE/WebSocket
- sqlx + SQLite for storage (file-backed, embedded)
- serde for serialization
- thiserror + anyhow for errors
- tracing + tracing-subscriber for logs
- React 18 + TypeScript + Vite for the UI
- rust-mavlink (or the best maintained Rust MAVLink crate) for MAVLink parsing (issue 4)

## 9. Scope

### In scope for v0.1

- Rust workspace with the crates above.
- Core domain model (mission run, events, telemetry samples, assertions, failure report).
- Deterministic demo failed run that produces `TAKEOFF_TIMEOUT`.
- SQLite storage with migrations and integration tests.
- Axum REST API for runs, events, assertions, failure reports, demo creation, and health.
- React + TS + Vite web UI: runs list, run detail, timeline, evidence cards, assertion table, telemetry overview, raw event log.
- MAVLink UDP ingestion (read-only) from PX4/ArduPilot SITL.
- YAML-based assertion engine that evaluates against normalized events and telemetry samples.
- Failure report generation with evidence.
- README and architecture docs sufficient for an external engineer to run the demo.
- Local-first execution: `cargo run` and `npm run dev` are enough.

### Explicitly out of scope for v0.1

- Sending control commands (arm, takeoff, mode change, mission upload). FlightTrace is read-only.
- Real-world flight on un-instrumented hardware.
- Cloud hosting, billing, multi-user accounts, auth, RBAC.
- Multi-tenant data isolation.
- Kubernetes, Kafka, NATS, ROS 2, TimescaleDB, ClickHouse.
- Companion computer agent.
- AI-generated diagnosis or LLM-based root cause analysis.
- Map / 3D replay (may come later; not required for v0.1).
- Mobile app, native desktop app, plugin SDK.
- Fleet management, scheduling, alerting, on-call.
- Arbitrary user code execution inside assertions.

## 10. Non-goals

- We are not building a flight log viewer for `.bin` / `.ulg` files. (Possible later, but not v0.1.)
- We are not competing with QGroundControl as a GCS.
- We are not building "yet another telemetry dashboard."
- We are not building a generic time-series database.
- We are not building observability infrastructure.

## 11. MVP phases

Implementation is organized into six small, sequential issues. Each issue keeps the project buildable and demoable.

| Issue | Title                                        | Outcome                                                                                    |
| ----- | -------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1     | Core domain model and demo run generator     | Rust workspace, domain types, deterministic failed demo run, unit tests. No DB, no UI.     |
| 2     | SQLite storage and Axum API                  | Persist runs/events/telemetry/assertions/failure reports. REST endpoints. Integration tests. |
| 3     | React UI                                     | Runs list, run detail, timeline, evidence, assertion table. "Create demo failed run" button. |
| 4     | MAVLink ingestion from SITL                  | Read-only MAVLink UDP ingestion. Real PX4/ArduPilot SITL produces real runs.                |
| 5     | Assertion engine and failure reports         | YAML assertions, evaluation against events/samples, structured failure reports.            |
| 6     | Docs, polish, and local demo                 | README, architecture diagram, SITL notes, troubleshooting, fmt/clippy/test green.           |

Issues 1–3 deliver a fully usable product on demo data. Issue 4 plugs in the real world. Issue 5 generalizes the assertion logic seeded in issue 1. Issue 6 makes it presentable.

## 12. Acceptance criteria for v0.1

- [ ] The project builds locally with `cargo build` and `npm run build`.
- [ ] Backend runs locally with a single command and binds to a documented port.
- [ ] Frontend runs locally with `npm run dev` and talks to the backend.
- [ ] User can click "Create demo failed run" and a new failed run is persisted.
- [ ] User can open the failed run in the UI and see status `failed` with reason `TAKEOFF_TIMEOUT`.
- [ ] The failure report shows evidence and a likely cause paragraph.
- [ ] The mission timeline shows the phase transitions of the demo run.
- [ ] The assertion table shows at least one failing takeoff assertion.
- [ ] The raw normalized event log is browsable.
- [ ] Core crates have unit tests; storage and API have integration tests.
- [ ] `cargo fmt`, `cargo clippy`, and `cargo test` all pass.
- [ ] The frontend builds without errors.
- [ ] README explains how to run the demo end-to-end.
- [ ] README explains how MAVLink SITL ingestion will be wired in.
- [ ] The product visibly feels like a **mission debugger**, not a telemetry dashboard.

## 13. Risks and mitigations

- **MAVLink crate churn.** The Rust MAVLink ecosystem is small. Mitigation: isolate MAVLink behind `flighttrace-mavlink`; the rest of the system depends on normalized events, not on the MAVLink crate.
- **Scope creep into "dashboard land."** Mitigation: every UI element must answer a debugging question ("why did this fail?", "what was the vehicle state?"). If it does not, it is cut.
- **Demo data drifting from real data.** Mitigation: demo runs go through the same domain types, the same storage, and the same assertion engine as MAVLink runs. The only difference is the source.
- **Premature abstraction.** Mitigation: each issue lists its scope explicitly and forbids work on future issues except small documented stubs.

## 14. Development process

1. Implement one issue at a time, in order.
2. Do not start a future issue unless the current one explicitly requires a small stub.
3. The project must build, lint, and test cleanly at the end of every issue.
4. Mark acceptance criteria as completed in the issue file as you go.
5. Update `docs/` when behavior changes.
6. Prefer simple architecture over clever abstractions.
7. Avoid large rewrites unless a previous decision is clearly broken.
