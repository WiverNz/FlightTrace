# FlightTrace

FlightTrace is a Rust-based, developer-first **mission debugger** for PX4 and ArduPilot autonomous missions. It records MAVLink telemetry, reconstructs the mission timeline, evaluates assertions, and explains why a mission failed with concrete evidence.

See `docs/prd.md` for the full product brief and `docs/issues/` for per-issue specs.

## Quick start

```bash
# API + UI backed by a local SQLite database (demo-only mode).
cargo run -p flighttrace-cli -- serve \
    --database ./data/flighttrace.db \
    --bind 127.0.0.1:8080

# Web UI (separate terminal).
cd web && npm install && npm run dev
```

Without `--mavlink`, the server only exposes the demo runs seeded by `flighttrace-demo`. The UI from issue 3 lives at the URL printed by `vite`.

## Connecting to PX4 / ArduPilot SITL

FlightTrace ingests MAVLink **read-only over UDP**. Point it at a SITL endpoint and a new `source = mavlink` run shows up in the UI as soon as the first HEARTBEAT arrives.

### Launch SITL with MAVLink on `udp://127.0.0.1:14550`

**PX4 SITL (gazebo classic)** — PX4's `mavlink` module already publishes to `udp://127.0.0.1:14550` for QGroundControl by default, so no extra flags are required:

```bash
# From your PX4-Autopilot checkout
make px4_sitl gazebo-classic
```

**ArduPilot SITL** — Copter example, with `--out` adding FlightTrace as an additional MAVLink listener (QGC stays on the default port):

```bash
# From your ardupilot checkout
sim_vehicle.py -v ArduCopter --console --map \
    --out=udpout:127.0.0.1:14550
```

If port 14550 is already in use by QGroundControl, pick another (e.g. `14560`) and pass that to FlightTrace.

### Start FlightTrace with MAVLink ingestion

```bash
cargo run -p flighttrace-cli -- serve \
    --database ./data/flighttrace.db \
    --bind 127.0.0.1:8080 \
    --mavlink udp://127.0.0.1:14550
```

Optional `--mavlink-dialect <common|ardupilotmega|px4>` (default `common`). PX4 also speaks `common`, so the default works for both stacks. Use `ardupilotmega` if you want ArduPilot's dialect extensions decoded.

You should see, in order:

```
MAVLink ingestion starting endpoint=udp://127.0.0.1:14550 …
MAVLink ingestion bound; waiting for HEARTBEAT
opened MAVLink mission run …
```

Stop with Ctrl+C. The active run is closed cleanly (`status = Succeeded` unless a `MissionFailed` event was recorded during the session).

### Read-only guarantee

FlightTrace **never** writes to the MAVLink socket. It does not arm, disarm, change mode, upload missions, set parameters, or send any other command. The `flighttrace-mavlink` crate exposes no send function, and a CI test (`tests/read_only_invariant.rs`) scans the crate's source for forbidden send-related APIs on every build. Treat FlightTrace as a passive observer that you can safely attach to a live vehicle.

## Workspace layout

```
crates/
  flighttrace-core/       # normalized event + telemetry types
  flighttrace-storage/    # SQLite repos (sqlx)
  flighttrace-api/        # Axum HTTP API
  flighttrace-demo/       # seed demo runs
  flighttrace-mavlink/    # read-only MAVLink UDP ingestion
  flighttrace-cli/        # `flighttrace serve` binary
web/                      # React UI (Vite + TS)
docs/                     # PRD + per-issue specs
```

## Quality gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm run build && npm run lint
```
