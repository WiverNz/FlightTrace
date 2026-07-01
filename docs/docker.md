# Docker

**Backend-only. Optional. Not the primary way to run FlightTrace.**

The supported development workflows are the four modes documented in
[`run-modes.md`](run-modes.md). The Docker image in the repo root is
a convenience for people who want to sandbox the API without
installing a Rust toolchain — nothing more.

## What it builds

The root `Dockerfile` builds the Rust backend only. It does **not**
build or serve the web frontend and does not ship a Tauri shell. The
image exposes the same HTTP surface documented in [`api.md`](api.md),
listening on `0.0.0.0:8080`.

There is no `docker-compose.yml` in v0.1. Static web serving is
handled by `flighttrace serve --web-dir …` (packaged web mode); adding
Compose to orchestrate two containers would duplicate that.

## Build and run

```bash
docker build -t flighttrace-backend .
docker run --rm \
    --publish 8080:8080 \
    --volume flighttrace-data:/data \
    flighttrace-backend
```

The container stores its SQLite database under `/data/flighttrace.db`
via the named volume. Verify it started with
`curl http://127.0.0.1:8080/health`.

## Environment overrides

All `FLIGHTTRACE_*` variables from [`configuration.md`](configuration.md)
work inside the container. Common overrides:

```bash
docker run --rm \
    --publish 8080:8080 \
    --volume flighttrace-data:/data \
    --env FLIGHTTRACE_DATABASE=/data/flighttrace.db \
    --env RUST_LOG=info,flighttrace=debug \
    flighttrace-backend
```

MAVLink ingestion works too, provided the vehicle publishes to a UDP
port the container can reach. Add `--network host` (Linux) or map the
UDP port explicitly if you need to receive `HEARTBEAT`s from the host.

## Frontend

Serve the SPA yourself — either

- point a browser at a separate `npm run dev` (dev mode), or
- build `web/dist` and serve it from any static host that can proxy
  `/api/*` to the container.

Or don't bother with Docker at all and use packaged web mode
(`flighttrace serve --web-dir ./web/dist --open`) — that path is
simpler and gets you the whole product in one process.
