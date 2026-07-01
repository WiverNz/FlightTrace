# Tech Stack

## Backend

* **Rust** — core backend logic, domain model, mission analysis, MAVLink ingestion, and API.
* **Cargo workspace** — modular project structure split into focused crates:

  * `flighttrace-core` — shared domain types: mission runs, normalized events, telemetry samples, assertions, failure reports.
  * `flighttrace-demo` — deterministic demo mission generator, including the `TAKEOFF_TIMEOUT` scenario.
  * `flighttrace-storage` — SQLite persistence layer using `sqlx`.
  * `flighttrace-api` — Axum HTTP API and static web serving.
  * `flighttrace-mavlink` — read-only MAVLink UDP ingestion from PX4 / ArduPilot SITL or hardware.
  * `flighttrace-assertions` — YAML mission assertion engine and failure report generation.
  * `flighttrace-cli` — command-line entrypoint for `demo` and `serve`.
* **Tokio** — async runtime for HTTP server, MAVLink ingestion, shutdown handling, and background tasks.
* **Axum** — HTTP API server and static frontend serving.
* **SQLite + sqlx** — local-first persistence for mission runs, normalized events, telemetry samples, assertion results, and failure reports.
* **tower-http** — CORS, tracing middleware, and static file serving.
* **tracing** — structured logging for backend, API, storage, and MAVLink ingestion.
* **MAVLink / rust-mavlink** — read-only telemetry ingestion from PX4 and ArduPilot.
* **serde / serde_json / serde_yaml** — JSON API DTOs, event payloads, and YAML mission assertions.
* **clap** — CLI argument parsing with support for local configuration via flags and environment variables.

## Frontend

* **React 18** — web UI for browsing and debugging mission runs.
* **TypeScript** — typed API DTOs and safer UI state handling.
* **Vite** — fast frontend development and production build.
* **React Router** — routes for run list and run detail pages.
* **Plain CSS** — custom dark debugger-style UI without Tailwind or a heavy design system.
* **Fetch API** — simple typed API client; no React Query, Redux, GraphQL, or generated client.
* **Static build served by Axum** — production-style mode where the Rust backend serves `web/dist` directly.

## Product Architecture

FlightTrace is designed as a **local-first mission debugger**, not a telemetry dashboard.

```text
PX4 / ArduPilot SITL or demo data
        ↓
MAVLink / deterministic demo source
        ↓
Normalized events + telemetry samples
        ↓
Assertion engine
        ↓
Failure report with evidence
        ↓
React debugger UI
```

The main product loop is:

1. Create or ingest a mission run.
2. Normalize raw data into structured mission events.
3. Evaluate mission assertions.
4. Generate a failure report.
5. Show why the mission failed.

## Local Data Model

Core entities:

* **Mission run** — one execution of a mission.
* **Mission timeline** — ordered mission phase transitions.
* **Normalized event** — vehicle-agnostic event such as `HeartbeatReceived`, `ArmedChanged`, `AltitudeUpdated`, `MissionFailed`.
* **Telemetry sample** — timestamped altitude, vertical speed, mode, armed state, battery data.
* **Assertion result** — pass / warn / fail result from mission expectations.
* **Failure report** — structured explanation with reason, evidence, and likely cause.

## Mission Assertions

Assertions are written in YAML.

Example:

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

Supported v0.1 checks:

* `heartbeat_within_sec`
* `armed`
* `altitude_at_least_m`
* `mode_not_changes`
* `no_telemetry_gap_longer_than_sec`

## Design System

### Visual Style

Dense, dark, developer-first debugger UI.

The interface should feel closer to:

* tracing tools,
* log viewers,
* flight debugging tools,
* incident analysis tools,

and not like a generic SaaS dashboard.

### Typography

Recommended:

* **Inter / system sans-serif** — body and UI text.
* **JetBrains Mono / IBM Plex Mono / ui-monospace** — timestamps, IDs, raw events, payloads, assertions.
* Optional display font later, but not required for v0.1.

### Color Palette

Dark technical palette with high readability.

```css
:root {
  --bg: #090d14;
  --surface: #111722;
  --surface-raised: #161d2a;
  --border: #273244;

  --text: #e6edf7;
  --text-muted: #9aa8bd;
  --text-dim: #6f7d91;

  --accent: #6ea8fe;
  --danger: #ff5c6c;
  --warning: #f5c66b;
  --success: #39d98a;

  --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}
```

Status colors:

```css
.status-failed {
  color: var(--danger);
}

.status-succeeded {
  color: var(--success);
}

.status-running {
  color: var(--accent);
}
```

## Run Detail UI

The main screen should explain the mission failure quickly.

Recommended layout:

```text
Run Header
  mission, vehicle, source, status, duration

Failure Summary
  expected altitude
  max reached altitude
  altitude gap
  last warning
  failure time

Failure Report
  human-readable evidence
  likely cause

Mission Timeline
  phase transitions with relative timestamps

Assertions
  failed first, then warnings, then passing checks

Telemetry Overview
  altitude chart
  vertical speed
  sample table

Raw Event Log
  filterable normalized events
  raw payloads
```

For the demo `TAKEOFF_TIMEOUT` run, the screen should communicate:

```text
The vehicle was expected to reach 10.0 m,
but only reached 2.4 m and stopped climbing
after an EKF altitude variance warning.
```

## Runtime Modes

### Development mode

Backend:

```bash
cargo run -p flighttrace-cli -- serve \
  --bind 127.0.0.1:8080 \
  --database ./data/flighttrace.db
```

Frontend:

```bash
cd web
npm install
npm run dev
```

Open:

```text
http://localhost:5173
```

### Packaged local mode

Build frontend:

```bash
cd web
npm run build
```

Run backend with static UI:

```bash
cargo run -p flighttrace-cli -- serve \
  --bind 127.0.0.1:8080 \
  --database ./data/flighttrace.db \
  --web-dir ./web/dist \
  --open
```

Open:

```text
http://127.0.0.1:8080
```

### MAVLink SITL mode

ArduPilot example:

```bash
sim_vehicle.py -v ArduCopter --out=udp:127.0.0.1:14550
```

FlightTrace:

```bash
flighttrace serve \
  --bind 127.0.0.1:8080 \
  --database ./data/flighttrace.db \
  --web-dir ./web-dist \
  --mavlink udp://127.0.0.1:14550 \
  --mission examples/missions/simple_takeoff.yaml \
  --open
```

## Safety Model

FlightTrace v0.1 is **read-only** for MAVLink.

It does not:

* arm vehicles,
* take off,
* land,
* upload missions,
* change modes,
* write parameters,
* send commands.

It only listens, records, normalizes, evaluates, and explains.

## Release Packaging

Release artifacts should include:

```text
flighttrace / flighttrace.exe
web-dist/
README.md
LICENSE-MIT
LICENSE-APACHE
SHA256SUMS
```

Recommended release targets:

* `x86_64-unknown-linux-gnu`
* `x86_64-pc-windows-msvc`

Optional later:

* macOS builds
* Tauri desktop packaging
* single installer

## Minimal Recipe

To build a similar app:

1. Create a Rust workspace:

```bash
cargo new flighttrace --bin
```

Then convert it into a workspace with crates:

```text
crates/
  flighttrace-core
  flighttrace-storage
  flighttrace-api
  flighttrace-cli
```

2. Define the domain model first:

```rust
pub struct MissionRun {
    pub id: MissionRunId,
    pub mission: String,
    pub vehicle: String,
    pub status: MissionStatus,
    pub failure_reason: Option<FailureReason>,
}
```

3. Add a deterministic demo run before real integrations:

```rust
let bundle = flighttrace_demo::generate_failed_takeoff_run();
```

4. Persist runs locally with SQLite:

```text
mission_runs
normalized_events
telemetry_samples
assertion_results
failure_reports
```

5. Expose a small REST API:

```text
GET  /health
POST /api/demo/run
GET  /api/runs
GET  /api/runs/:id
GET  /api/runs/:id/events
GET  /api/runs/:id/telemetry
GET  /api/runs/:id/assertions
GET  /api/runs/:id/failure-report
POST /api/runs/:id/reevaluate
```

6. Build a focused React UI:

```text
Runs list
Run detail
Failure report
Mission timeline
Assertions
Telemetry overview
Raw event log
```

7. Add static serving from Axum:

```text
/api/*      → JSON API
/assets/*   → frontend assets
/runs/:id   → SPA fallback to index.html
/           → React app
```

8. Add MAVLink ingestion only after the demo loop works.

9. Keep real vehicle support read-only.

10. Package as:

```bash
flighttrace serve --web-dir ./web-dist --open
```

## Development Quality Gates

Rust:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Frontend:

```bash
cd web
npm run build
npm run lint
```

Release:

```bash
git tag -a v0.1.0 -m "FlightTrace v0.1.0"
git push origin v0.1.0
```
