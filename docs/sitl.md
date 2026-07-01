# SITL quickstarts

FlightTrace is a **read-only** MAVLink observer. Attach it to
ArduPilot's `sim_vehicle.py` or PX4's SITL alongside the ground-station
you'd normally use; it ingests the same UDP stream and evaluates the
mission YAML against the recorded events on session close.

For the read-only guarantee and the assertion engine details, see the
top-level [README](../README.md#read-only-guarantee).

## ArduPilot

`sim_vehicle.py` already publishes MAVLink; add a `--out` line so
FlightTrace gets its own listener (QGroundControl, if you use it,
stays on the default port).

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

# Terminal 3 — Vite dev server (or use packaged web mode from README).
cd web && npm run dev
```

A new run with `source = mavlink` appears in the UI as soon as the
first `HEARTBEAT` arrives. Use MAVProxy / QGroundControl to arm, take
off, and fly. When the session ends (inactivity timeout or Ctrl+C),
the mission spec is evaluated and the run is finalised as `succeeded`
or `failed` with a `FailureReport`.

`--mavlink-dialect ardupilotmega` decodes ArduPilot's dialect
extensions; the default `common` works for both stacks.

## PX4

PX4's `mavlink` module publishes to `udp://127.0.0.1:14550` for
QGroundControl by default — FlightTrace can attach to the same
endpoint.

```bash
# Terminal 1 — PX4 SITL (from your PX4-Autopilot checkout).
make px4_sitl gazebo-classic

# Terminal 2 — FlightTrace with MAVLink ingestion.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db \
    --mavlink udp://127.0.0.1:14550 \
    --mission examples/missions/simple_takeoff.yaml

# Terminal 3 — Vite dev server (or use packaged web mode from README).
cd web && npm run dev
```

PX4 speaks the `common` dialect so the default `--mavlink-dialect`
is fine.

## Port conflicts

If port `14550` is already occupied by QGroundControl, configure PX4
(or ArduPilot) to publish to a free port (for example `14560`) and
pass that to FlightTrace:

```bash
--mavlink udp://127.0.0.1:14560
```

If FlightTrace's own `:8080` bind clashes with something else, see
[`configuration.md`](configuration.md) for `FLIGHTTRACE_BIND` and
`--bind`.

## Assertion outcomes

The mission YAML in `examples/missions/simple_takeoff.yaml` is the
same spec the deterministic demo run uses. The v0.1 check set is
documented in the top-level [README](../README.md#mission-assertions);
the JSON shape returned by the API is in [`api.md`](api.md).
