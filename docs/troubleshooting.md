# Troubleshooting

## Ports

**Port 8080 already in use (`Address already in use`).** Find the
listener with `lsof -i :8080` or `ss -ltnp | grep 8080`, then stop it
if it's safe. Alternatively set `FLIGHTTRACE_BIND=127.0.0.1:8088` or
pass `--bind 127.0.0.1:8088`. If you're using cross-origin dev mode,
update `web/.env.development`'s `VITE_API_BASE_URL` to match.

**Port 5173 already in use.** Vite will offer the next free port and
print it. If you pin a custom port, update the CORS allowlist in
`crates/flighttrace-api/src/router.rs` (`DEFAULT_CORS_ORIGIN`) and
rebuild the backend.

## UI ↔ API mismatches

**No runs appear in the UI.** Confirm the API is running with
`curl http://127.0.0.1:8080/health`. Check the browser dev console for
successful `GET /api/runs` calls. In dev mode, verify
`web/.env.development` points at the same address the API bound to.
In packaged web or desktop mode, both are same-origin — nothing to
configure.

**Frontend type errors after API changes.** The TypeScript DTOs in
`web/src/types/api.ts` are hand-written mirrors of the Rust DTOs. If a
field is renamed on the Rust side, update the TS side in lockstep.

## MAVLink ingestion

**No heartbeats.** Verify the endpoint (`udp://127.0.0.1:14550` by
default). Check no other consumer has bound the same port. Confirm
the SITL is publishing:

- PX4: `mavlink status` in the nsh console.
- ArduPilot: `sim_vehicle.py`'s own console.

On WSL2 / Windows make sure the firewall allows UDP loopback. Try
`--mavlink-dialect ardupilotmega` if your stack uses dialect
extensions. See [`sitl.md`](sitl.md) for the full quickstart shape.

## SQLite

**Database is locked.** Only one backend process can write the SQLite
file at a time. Stop any other `flighttrace serve` (or the desktop
shell) that points at the same `--database` path, or point each to a
separate one.

## Toolchain

**Compilation errors after pulling.** Run `cargo clean` and re-run
`cargo build --workspace`. The workspace requires a toolchain that
supports `edition = "2024"` (MSRV ≥ 1.85).

**Desktop crate fails with `resource path "../../../web/dist" doesn't
exist`.** Tauri's build script validates `bundle.resources` at compile
time. Run `cd web && npm run build && cd ..` before any desktop-crate
cargo command. See [`desktop.md`](desktop.md).

## WSL2 rendering

`libEGL warning: failed to get driver name for fd -1`,
`MESA-LOADER: failed to retrieve device information`, and
`ZINK: failed to choose pdev` from `cargo tauri dev` are cosmetic —
WebKit falls back to software rendering and the window works fine.
Silence them with:

```bash
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
```

Both are commented out in `.env.example`. See [`desktop.md`](desktop.md)
for context.
