# FlightTrace

[![CI](https://github.com/WiverNz/FlightTrace/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/WiverNz/FlightTrace/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

> A developer-first **mission debugger** that explains **why** autonomous
> PX4/ArduPilot missions fail, with evidence.

FlightTrace records MAVLink telemetry from PX4 or ArduPilot (SITL or real
hardware), reconstructs the mission timeline as normalized events and
telemetry samples, evaluates declarative YAML assertions against the run, and
produces a structured failure report with cited evidence.

It is local-first (one Rust backend, one React frontend, one SQLite file on
disk), strictly **read-only** on the vehicle, and ships with a deterministic
demo run so you can evaluate the product without installing PX4 or ArduPilot.

## What FlightTrace is

- a mission debugger for PX4 and ArduPilot
- a structured failure-analysis tool with cited evidence
- a local, single-user, single-binary workflow (Rust API + React UI + SQLite)
- a read-only MAVLink consumer

## What FlightTrace is not

- not a telemetry dashboard
- not an AirData / Flight Review clone
- not a ground control station — it cannot arm, disarm, or change mode
- not a fleet-management or ops platform
- not a generic time-series database
- not a cloud product, not multi-user, not multi-tenant
- not an AI auto-diagnosis tool

## How it works

1. **Ingest.** `flighttrace-mavlink` reads MAVLink UDP packets from SITL or a
   real vehicle. The demo source produces the same shape of data without a
   vehicle.
2. **Normalize.** Each MAVLink message is translated into a vehicle-agnostic
   `NormalizedEvent` (e.g. `ArmedChanged`, `AltitudeUpdated`,
   `TelemetryGapDetected`) plus periodic `TelemetrySample`s. Everything
   downstream consumes these — the source is interchangeable.
3. **Evaluate.** When a mission run ends (or on demand via the re-evaluate
   API), `flighttrace-assertions` runs the YAML mission spec against the
   recorded events and samples and emits per-step `AssertionResult`s plus, on
   failure, a `FailureReport` with reason, evidence, and a likely cause.

## Architecture

```
+-----------------------------+        +--------------------------+
|  React + TypeScript + Vite  | <----> |  Axum HTTP API (Rust)    |
|  web/                       |  REST  |  flighttrace-api         |
+-----------------------------+        +--------------------------+
                                                   |
                                                   v
                            +------------------------------------------+
                            |  flighttrace-core         (domain types) |
                            |  flighttrace-storage      (sqlx + SQLite)|
                            |  flighttrace-assertions   (YAML engine)  |
                            |  flighttrace-demo         (seed run)     |
                            |  flighttrace-mavlink      (UDP ingest)   |
                            +------------------------------------------+
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
                                       | via MAVLink UDP (read)   |
                                       +--------------------------+
```

The Rust workspace is six small crates plus a CLI binary (`flighttrace-cli`).
Each crate has one responsibility and one public surface; `flighttrace-core`
owns the domain types and nothing else redefines them.

## Quickstart — demo only

No PX4 or ArduPilot installation is needed for this path. The demo source
produces a deterministic failed run that exercises the full pipeline.

Prereqs: a stable Rust toolchain (workspace uses `edition = "2024"`) and
Node.js ≥ 18 with `npm`.

```bash
# Terminal 1 — API + SQLite (demo-only mode, no MAVLink).
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db

# Terminal 2 — Vite dev server.
cd web
npm install
npm run dev
```

Then:

1. Open <http://localhost:5173>.
2. Click **Create demo failed run** in the top right of the runs list.
3. The new row appears at the top, badged `FAILED — TAKEOFF_TIMEOUT`. Click
   it.
4. The run detail page shows the failure report card (`Mission failed:
   TAKEOFF_TIMEOUT`, evidence list, likely cause), the mission timeline, the
   assertion table, the telemetry overview, and the raw event log.

The run is persisted in `./data/flighttrace.db` and survives restarts.

### Local configuration

Every `serve` option can also come from the environment. Resolution order is
**CLI flag > environment variable > built-in default**:

| Environment variable | CLI flag | Default |
| --- | --- | --- |
| `FLIGHTTRACE_BIND` | `--bind` | `127.0.0.1:8080` |
| `FLIGHTTRACE_DATABASE` | `--database` | `./data/flighttrace.db` |
| `FLIGHTTRACE_MAVLINK` | `--mavlink` | unset |
| `FLIGHTTRACE_MAVLINK_DIALECT` | `--mavlink-dialect` | `common` |
| `FLIGHTTRACE_MISSION` | `--mission` | unset |
| `FLIGHTTRACE_WEB_DIR` | `--web-dir` | unset (API-only) |
| `FLIGHTTRACE_OPEN` | `--open` | unset |

`RUST_LOG` controls tracing output and `RUST_BACKTRACE` has its normal Rust
meaning. Copy the example and export it into the current shell before running
the CLI (`clap` reads the environment; it does not load `.env` itself):

```bash
cp .env.example .env
set -a
source .env
set +a
cargo run -p flighttrace-cli -- serve
```

For example, `--bind 127.0.0.1:8089` overrides
`FLIGHTTRACE_BIND=127.0.0.1:8088`. The root `.env` is gitignored, while
`.env.example` is safe to commit.

VS Code launch configurations read the same root `.env` file. Its example
uses debug bind address `127.0.0.1:8088`, leaving the README quickstart on
port `8080`. MAVLink and mission variables are commented out by default, so
the `CLI: serve demo-only` profile remains demo-only; the MAVLink profile
supplies them as CLI flags.

### Run modes

FlightTrace has three run modes; pick the one that fits the task.

- **CLI dev (two processes).** `cargo run … serve` on `:8080` and
  `npm run dev` on `:5173`. Best for frontend iteration; Vite's HMR is
  live. This is the workflow used in the demo quickstart above.
- **CLI packaged (one process).** Build the frontend once, then let
  `flighttrace serve` also serve the bundle from the same origin. Best
  for evaluation, screenshots, and shipping a single binary alongside a
  `web-dist/` folder.
- **Desktop (Tauri, one process).** A thin Tauri v2 shell that spawns
  the same Axum server on a random loopback port and points a native
  WebView at it. Same API, same React bundle — no Electron. See below.

**CLI packaged mode**:

```bash
# One-time frontend build.
cd web && npm ci && npm run build && cd ..

# One process serves both the API and the UI.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db \
    --web-dir ./web/dist \
    --open
```

Then open <http://127.0.0.1:8080>. `--open` launches the default browser
once the listener binds — drop it in headless environments. Client-side
routes like `/runs/<id>` fall back to `index.html`, so React Router
still owns them; unknown `/api/*` paths still return the JSON 404
envelope. When `--web-dir` is omitted the API-only behaviour is
identical to previous releases. The frontend uses same-origin fetches
in this mode — nothing to configure.

**Desktop mode (Tauri):**

```bash
# One-time frontend build.
cd web && npm ci && npm run build && cd ..

# One-time tauri-cli install.
cargo install tauri-cli --version '^2'

# Native window with the FlightTrace UI.
cd apps/flighttrace-desktop/src-tauri
cargo tauri dev
```

The shell binds Axum on `127.0.0.1:0` (a free loopback port) and points
the WebView at that URL. Closing the window flips a graceful-shutdown
watch and the process exits within a fraction of a second. SQLite lives
at `./data/flighttrace-desktop.db` by default; set `FLIGHTTRACE_DATABASE`
to override. Set `FLIGHTTRACE_WEB_DIR` if you move the binary away from
the repo checkout — v0.1 doesn't auto-bundle `web/dist` into the Tauri
build yet.

The desktop crate is intentionally **not** a workspace member: on Linux
it needs webkit2gtk + GTK + libglib link-time deps that aren't required
for the CLI. Install on Debian/Ubuntu:

```bash
sudo apt install \
    libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev \
    libayatana-appindicator3-dev librsvg2-dev pkg-config \
    build-essential curl wget file libssl-dev
```

macOS and Windows don't need extra system packages beyond the standard
Tauri prerequisites — see [Tauri v2's prerequisites
guide](https://v2.tauri.app/start/prerequisites/) for the current list.

## Quickstart — ArduPilot SITL

ArduPilot's `sim_vehicle.py` already publishes MAVLink; add a `--out` line so
FlightTrace gets its own listener (QGroundControl, if you use it, stays on
the default port).

```bash
# Terminal 1 — ArduPilot SITL (from your ardupilot checkout).
sim_vehicle.py -v ArduCopter --console --map \
    --out=udpout:127.0.0.1:14550

# Terminal 2 — FlightTrace with MAVLink ingestion.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db \
    --mavlink udp://127.0.0.1:14550 \
    --mission examples/missions/simple_takeoff.yaml

# Terminal 3 — Vite dev server.
cd web && npm run dev
```

A new run with `source = mavlink` appears in the UI as soon as the first
`HEARTBEAT` arrives. Use MAVProxy / QGroundControl to arm, take off, and fly.
When the session ends (inactivity timeout or Ctrl+C), the mission spec is
evaluated and the run is finalised as `succeeded` or `failed` with a
`FailureReport`.

`--mavlink-dialect ardupilotmega` decodes ArduPilot's dialect extensions; the
default `common` works for both stacks.

## Quickstart — PX4 SITL

PX4's `mavlink` module publishes to `udp://127.0.0.1:14550` for
QGroundControl by default — FlightTrace can attach to the same endpoint.

```bash
# Terminal 1 — PX4 SITL (from your PX4-Autopilot checkout).
make px4_sitl gazebo-classic

# Terminal 2 — FlightTrace with MAVLink ingestion.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db \
    --mavlink udp://127.0.0.1:14550 \
    --mission examples/missions/simple_takeoff.yaml

# Terminal 3 — Vite dev server.
cd web && npm run dev
```

PX4 speaks the `common` dialect so the default `--mavlink-dialect` is fine.
If port `14550` is already occupied by QGroundControl, configure PX4 (or
ArduPilot) to publish to a free port (e.g. `14560`) and pass that to
FlightTrace.

### Read-only guarantee

FlightTrace **never** writes to the MAVLink socket. It does not arm, disarm,
change mode, upload missions, set parameters, or send any other command. The
`flighttrace-mavlink` crate exposes no send function, and an in-tree
integration test (`crates/flighttrace-mavlink/tests/read_only_invariant.rs`)
scans the crate's source for forbidden send-related APIs on every build.
Attach FlightTrace to a live vehicle as a passive observer.

## Mission assertions

A mission spec is a small YAML document describing the steps the vehicle is
expected to complete. The engine evaluates each step's `expect` block against
the recorded events and telemetry samples and emits one `AssertionResult` per
check (`pass`, `warn`, or `fail`).

`examples/missions/simple_takeoff.yaml` is the spec used by the demo run and
the SITL quickstarts:

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

The v0.1 checks are:

| Check                                | Meaning                                                                  | Failure reason         |
| ------------------------------------ | ------------------------------------------------------------------------ | ---------------------- |
| `heartbeat_within_sec: <n>`          | A `HeartbeatReceived` event lands within `n` seconds of the step window. | `NoHeartbeat`          |
| `armed: <bool>`                      | The vehicle reaches the expected armed state inside the step window.     | `CommandTimeout`       |
| `altitude_at_least_m: <m>`           | An `AltitudeUpdated` or telemetry sample meets `m` metres inside window. | `TakeoffTimeout`       |
| `mode_not_changes: true`             | No `ModeChanged` event fires inside the step window.                     | `UnexpectedModeChange` |
| `no_telemetry_gap_longer_than_sec`   | No `TelemetryGapDetected` event exceeds the budget inside the window.    | `TelemetryGap`         |

When any check fails, the engine emits a `FailureReport` with `summary =
"Mission failed: <REASON>"`, the union of evidence cited by failing checks
(events, samples, statustexts), and a one-paragraph likely-cause string.

There is no scripting language, no conditional logic, and no arbitrary user
code in the YAML — checks are declarative.

## HTTP API

The Axum server in `flighttrace-api` exposes:

| Method | Path                                  | Purpose                                                                 |
| ------ | ------------------------------------- | ----------------------------------------------------------------------- |
| GET    | `/health`                             | Liveness probe — `{"status":"ok","version":"…"}`.                       |
| GET    | `/api/runs?limit&offset`              | Mission runs, ordered by `startedAt DESC`.                              |
| GET    | `/api/runs/:id`                       | Full run header (status, failure reason, phases).                       |
| GET    | `/api/runs/:id/events`                | Normalized events for the run, time-ordered.                            |
| GET    | `/api/runs/:id/telemetry`             | Telemetry samples for the run, time-ordered.                            |
| GET    | `/api/runs/:id/assertions`            | Assertion results for the run.                                          |
| GET    | `/api/runs/:id/failure-report`        | Structured failure report, 404 if the run has none.                     |
| POST   | `/api/runs/:id/reevaluate`            | Re-runs the assertion engine. Body `{ "missionPath": "…" }` is optional. |
| POST   | `/api/demo/run`                       | Generates the deterministic demo failed run and returns `{ "id": "…" }`. |

All JSON uses `camelCase` field names. Errors return
`{ "error": { "code": "...", "message": "..." } }` and the documented HTTP
status. The dev CORS allowlist accepts `http://localhost:5173` and
`http://127.0.0.1:5173`.

## Project layout

```
flighttrace/
  Cargo.toml                          # workspace manifest
  crates/
    flighttrace-core/                 # domain types, ids, time, errors
    flighttrace-demo/                 # deterministic failed-run generator
    flighttrace-storage/              # sqlx + SQLite, migrations, repos
    flighttrace-api/                  # Axum REST API (+ CORS, tracing)
    flighttrace-mavlink/              # read-only MAVLink UDP ingestion
    flighttrace-assertions/           # YAML assertion engine + reports
    flighttrace-cli/                  # `flighttrace` binary
  web/                                # React + TypeScript + Vite UI
    src/
      pages/                          # RunsListPage, RunDetailPage
      components/                     # RunsTable, FailureReportCard, …
      api/                            # typed fetch client
      types/                          # TS mirror of the API DTOs
  examples/
    missions/
      simple_takeoff.yaml             # spec used by demo + SITL quickstarts
  docs/
    prd.md                            # product brief
    context.md                        # working conventions
    issues/                           # per-issue specs (v0.1 history)
    screenshots/                      # captured PNGs go here
  data/                               # SQLite database (gitignored)
```

## Backend Docker image

The root `Dockerfile` builds only the Rust backend. It does not build or serve
the web frontend. Build and run it with a persistent Docker volume for SQLite:

```bash
docker build -t flighttrace-backend .
docker run --rm \
    --publish 8080:8080 \
    --volume flighttrace-data:/data \
    flighttrace-backend
```

The container listens on `0.0.0.0:8080` and stores its database at
`/data/flighttrace.db`. Verify it with
`curl http://127.0.0.1:8080/health`.

This image is a backend-only packaging option. The Cargo and npm commands
below remain the primary development workflow.

## Development

Quality gates that must stay green:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run build
cd web && npm run lint
```

Useful one-offs:

```bash
# Inspect the deterministic demo bundle as JSON.
cargo run -p flighttrace-cli -- demo | head -n 40

# Start the API against a throwaway database.
cargo run -p flighttrace-cli -- serve \
    --database ./data/smoke.db \
    --bind 127.0.0.1:18181

# Generate a demo failed run and fetch its failure report.
curl -s -X POST http://127.0.0.1:18181/api/demo/run
curl -s http://127.0.0.1:18181/api/runs \
    | head -n 5
curl -s http://127.0.0.1:18181/api/runs/<id>/failure-report
```

## Screenshots

Captured images live under `docs/screenshots/` (a `.gitkeep` placeholder
keeps the directory in git). To produce a fresh set:

1. Start the demo (`cargo run -p flighttrace-cli -- serve …` + `npm run dev`).
2. Resize the browser to **1440×900** and use the dark theme that ships by
   default.
3. Click **Create demo failed run** so the runs list has at least one entry.
4. Capture:
   - `docs/screenshots/runs-list.png` — the runs list with the failed demo
     run at the top.
   - `docs/screenshots/run-detail.png` — the run detail page with the
     failure report card visible.
   - `docs/screenshots/failure-report.png` — a close-up of the failure
     report card with the evidence list expanded.
5. Keep each PNG under ~200 KB. Reference them inline in this README when
   they are committed.

## Troubleshooting

- **Port 8080 already in use (`Address already in use`).** Find the listener
  with `lsof -i :8080` or `ss -ltnp | grep 8080`, then stop it if it is safe
  to do so. Alternatively set `FLIGHTTRACE_BIND=127.0.0.1:8088` or pass
  `--bind 127.0.0.1:8088`. Update `web/.env.development`'s
  `VITE_API_BASE_URL` to match.
- **Port 5173 already in use.** Vite will offer the next port and print it.
  If you pin a custom port, update the CORS allowlist in
  `crates/flighttrace-api/src/router.rs` (`DEFAULT_CORS_ORIGIN`) and rebuild.
- **No runs appear in the UI.** Confirm the API is running
  (`curl http://127.0.0.1:8080/health`) and that the browser dev console
  shows successful `GET /api/runs` calls. Check that
  `web/.env.development` points at the same address the API bound to.
- **MAVLink listener sees no heartbeats.** Verify the endpoint
  (`udp://127.0.0.1:14550` by default), check that no other consumer has
  bound the same port, confirm the SITL is publishing (PX4: `mavlink status`
  in nsh; ArduPilot: `sim_vehicle.py` console), and on WSL2 / Windows make
  sure the firewall allows UDP loopback. Try `--mavlink-dialect
  ardupilotmega` if your stack uses dialect extensions.
- **Database is locked.** Only one backend process can write the SQLite file
  at a time. Stop any other `flighttrace serve` instance, or point each to a
  separate `--database` path.
- **Compilation errors after pulling.** Run `cargo clean` and re-run
  `cargo build --workspace`. The workspace requires a toolchain that
  supports `edition = "2024"`.
- **Frontend type errors after API changes.** The TS DTOs in
  `web/src/types/api.ts` are hand-written mirrors of the Rust DTOs. If a
  field is renamed on the Rust side, update the TS side in lockstep.

## Roadmap

Out of scope for v0.1, candidate post-v0.1 work:

- GitHub Actions CI for `fmt` / `clippy` / `test` / `npm build` on PRs.
- Single-binary release packaging: API serves the pre-built `web/dist`.
- Live UI streaming via SSE so MAVLink-sourced runs update without manual
  refresh.
- Additional assertion checks (`waypoint_reached_within_sec`,
  `battery_above_pct`, `gps_fix_at_least`).
- `.bin` / `.ulg` log import for post-flight debugging.
- Multi-vehicle sessions keyed on MAVLink `system_id`.

Explicitly out of scope, no plans: cloud hosting, multi-user accounts, auth,
RBAC, billing, Kubernetes, Kafka/NATS, ROS 2, TimescaleDB, ClickHouse,
companion-computer agents, command-sending paths, AI/LLM root-cause analysis.

### A note on Docker

No `docker-compose.yml` ships with v0.1 on purpose: the supported workflow is
`cargo run` + `npm run dev` against a local SQLite file, and a meaningful
container build is gated on the single-binary release packaging above.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option. Workspace metadata declares
`license = "MIT OR Apache-2.0"`.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
