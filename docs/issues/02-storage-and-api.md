# Issue 2 — SQLite storage and Axum API

## Summary

Add persistent storage and an HTTP API to FlightTrace. Mission runs, normalized events, telemetry samples, assertion results, and failure reports are stored in a local SQLite database via `sqlx`. An Axum server exposes a REST API that lists and inspects runs and that creates a demo failed run on demand. After this issue the demo bundle from issue 1 can be persisted and queried through HTTP — no UI yet.

## Product goal

After this issue, an engineer can:

1. Run `cargo run -p flighttrace-cli -- serve` (or equivalent) to start the backend.
2. `curl -X POST http://localhost:8080/api/demo/run` to create a new demo failed run.
3. `curl http://localhost:8080/api/runs` to see the run in a list.
4. `curl http://localhost:8080/api/runs/<id>/failure-report` to see the structured `TAKEOFF_TIMEOUT` report.
5. Restart the server and the run is still there.

This is the backend half of the v0.1 demo loop.

## Technical scope

- New crate `flighttrace-storage`: sqlx + SQLite, migrations, typed repositories.
- New crate `flighttrace-api`: Axum HTTP server, JSON handlers, error mapping.
- Extend `flighttrace-cli` with a `serve` subcommand that starts the API.
- Wire `POST /api/demo/run` to `flighttrace_demo::generate_failed_takeoff_run()`, persisting the resulting bundle.
- Integration tests covering migrations, repositories, and the full HTTP surface.

No frontend in this issue. No MAVLink.

## Files and modules to create or modify

```
crates/
  flighttrace-storage/
    Cargo.toml
    migrations/
      0001_init.sql                              # schema for runs/events/telemetry/assertions/reports
    src/
      lib.rs
      pool.rs                                    # connect, migrate, build SqlitePool
      runs.rs                                    # MissionRunRepo
      events.rs                                  # NormalizedEventRepo
      telemetry.rs                               # TelemetrySampleRepo
      assertions.rs                              # AssertionResultRepo
      failure.rs                                 # FailureReportRepo
      bundle.rs                                  # persist + load a full run bundle
      error.rs                                   # StorageError (thiserror)
  flighttrace-api/
    Cargo.toml
    src/
      lib.rs
      state.rs                                   # AppState { storage }
      router.rs                                  # all routes mounted here
      error.rs                                   # ApiError -> IntoResponse
      handlers/
        health.rs                                # GET /health
        runs.rs                                  # list, get, events, assertions, failure-report
        demo.rs                                  # POST /api/demo/run
      dto.rs                                     # camelCase JSON DTOs at the API boundary
  flighttrace-cli/
    src/main.rs                                  # add `serve` subcommand
docs/
  prd.md                                         # (no changes expected, but update if API drifts)
```

## Detailed implementation steps

1. **`flighttrace-storage` crate.**
   - Dependencies: `sqlx` (features: `runtime-tokio-rustls`, `sqlite`, `macros`, `migrate`, `time` or `chrono` to match the choice in issue 1, `uuid`), `flighttrace-core`, `thiserror`, `tracing`, `serde_json`.
   - In `pool.rs`, expose `pub async fn connect(database_url: &str) -> Result<SqlitePool, StorageError>` that:
     - creates the database file if missing,
     - sets `PRAGMA journal_mode = WAL`,
     - sets `PRAGMA foreign_keys = ON`,
     - runs `sqlx::migrate!("./migrations")`.
   - Write `migrations/0001_init.sql` with tables:
     - `mission_runs(id TEXT PRIMARY KEY, vehicle TEXT NOT NULL, mission TEXT NOT NULL, source TEXT NOT NULL, status TEXT NOT NULL, failure_reason TEXT, started_at TEXT NOT NULL, ended_at TEXT)`
     - `mission_phase_transitions(run_id TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE, at TEXT NOT NULL, phase TEXT NOT NULL, ordinal INTEGER NOT NULL, PRIMARY KEY (run_id, ordinal))`
     - `normalized_events(id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE, at TEXT NOT NULL, kind TEXT NOT NULL, payload TEXT NOT NULL)` — `payload` stores the variant-specific data as JSON.
     - `telemetry_samples(run_id TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE, at TEXT NOT NULL, altitude_m REAL NOT NULL, vertical_speed_mps REAL NOT NULL, mode TEXT NOT NULL, armed INTEGER NOT NULL, battery_remaining_pct REAL NOT NULL, PRIMARY KEY (run_id, at))`
     - `assertion_results(id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE, name TEXT NOT NULL, outcome TEXT NOT NULL, detail TEXT NOT NULL, evaluated_at TEXT NOT NULL)`
     - `failure_reports(id TEXT PRIMARY KEY, run_id TEXT NOT NULL UNIQUE REFERENCES mission_runs(id) ON DELETE CASCADE, reason TEXT NOT NULL, summary TEXT NOT NULL, evidence_json TEXT NOT NULL, likely_cause TEXT NOT NULL, generated_at TEXT NOT NULL)`
   - Add an index on `normalized_events(run_id, at)` and on `telemetry_samples(run_id, at)`.
   - In `runs.rs`, implement `MissionRunRepo` with:
     - `insert(run: &MissionRun) -> Result<()>`
     - `list(limit: u32, offset: u32) -> Result<Vec<MissionRun>>` ordered by `started_at DESC`
     - `get(id: MissionRunId) -> Result<Option<MissionRun>>`
   - Implement equivalents for events, telemetry, assertions, and failure reports.
   - In `bundle.rs`, expose `persist_bundle(pool, bundle) -> Result<MissionRunId>` and `load_bundle(pool, id) -> Result<Option<RunBundle>>`. Use a single SQLite transaction for `persist_bundle`.
   - Define `StorageError` with variants for `NotFound`, `Sqlx(#[from] sqlx::Error)`, `Migrate(#[from] sqlx::migrate::MigrateError)`, `Serialization(#[from] serde_json::Error)`.

2. **`flighttrace-api` crate.**
   - Dependencies: `axum` (with `macros`, `json`), `tokio` (`macros`, `rt-multi-thread`), `tower-http` (`trace`, `cors`), `serde`, `serde_json`, `flighttrace-core`, `flighttrace-storage`, `flighttrace-demo`, `tracing`, `thiserror`, `time` or `chrono`.
   - `AppState` holds an `Arc<SqlitePool>` (or `Arc<StorageHandles>`).
   - `dto.rs` defines `#[serde(rename_all = "camelCase")]` DTOs that mirror the domain types but live at the API boundary. Convert at the handler edge.
   - Routes (all under `axum::Router`):
     - `GET /health` → `200 {"status":"ok","version":"<crate version>"}`
     - `GET /api/runs?limit&offset` → list of `MissionRunDto`
     - `GET /api/runs/:id` → full `MissionRunDto`, 404 if missing
     - `GET /api/runs/:id/events` → list of `NormalizedEventDto`
     - `GET /api/runs/:id/assertions` → list of `AssertionResultDto`
     - `GET /api/runs/:id/failure-report` → `FailureReportDto`, 404 if the run has no report
     - `POST /api/demo/run` → calls `flighttrace_demo::generate_failed_takeoff_run()`, rewrites `started_at` to `now()` (and shifts the rest of the timeline by the same delta), persists the bundle, and returns `{ "id": "<MissionRunId>" }` with HTTP 201.
   - `error.rs` maps `StorageError::NotFound` to 404, `serde_json::Error` to 400 if parsing fails on input, everything else to 500. Always return JSON: `{ "error": { "code": "...", "message": "..." } }`.
   - Add a permissive CORS layer for `http://localhost:5173` (Vite default) so issue 3 can call the API.
   - Add `tower_http::trace::TraceLayer` for request/response tracing.

3. **`flighttrace-cli` updates.**
   - Add `serve` subcommand with flags `--bind` (default `127.0.0.1:8080`) and `--database` (default `./data/flighttrace.db`).
   - On startup: ensure the parent directory exists, connect to the pool, run migrations, start the Axum server. Log the bound address.

4. **Configuration.**
   - Accept the database path and bind address via CLI flags. No `.env` parsing in v0.1; keep it simple.
   - Default database path: `./data/flighttrace.db`. Add `./data/` to `.gitignore`.

5. **Integration tests.**
   - In `flighttrace-storage/tests/`, use `sqlx::SqlitePool::connect(":memory:")` (or a temp file) and run migrations. Tests:
     - Round-trip persistence of a `MissionRun` plus events, telemetry, assertions, and failure report.
     - `MissionRunRepo::list` returns runs in `started_at DESC` order.
     - Deleting a run cascades to its events, samples, assertions, and report.
   - In `flighttrace-api/tests/`, use `axum::Router` with an in-memory storage pool and `tower::ServiceExt::oneshot` to exercise:
     - `GET /health` returns 200 and `status: "ok"`.
     - `POST /api/demo/run` returns 201 with an id, and a subsequent `GET /api/runs` includes that id.
     - `GET /api/runs/:id/failure-report` returns a report with `reason == "TAKEOFF_TIMEOUT"`.
     - `GET /api/runs/<bogus uuid>` returns 404 with the documented error body.

## Acceptance criteria

- [x] `flighttrace-storage` crate compiles, exposes typed repositories, and runs its embedded migrations on connect.
- [x] `flighttrace-api` crate compiles and serves the documented routes.
- [x] `cargo run -p flighttrace-cli -- serve` starts the server on `127.0.0.1:8080` by default.
- [x] `POST /api/demo/run` persists a demo failed run and returns its id with HTTP 201.
- [x] `GET /api/runs` returns a list including the just-created run.
- [x] `GET /api/runs/:id/failure-report` returns the `TAKEOFF_TIMEOUT` report including evidence and likely cause.
- [x] All API JSON uses `camelCase` field names.
- [x] All API errors are JSON with shape `{ "error": { "code": "...", "message": "..." } }`.
- [x] CORS allows `http://localhost:5173`.
- [x] `./data/` is gitignored; the DB file is created on first run.
- [x] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass.

## Tests required

- Storage round-trip for the full run bundle.
- Listing order and pagination boundaries (limit/offset).
- ON DELETE CASCADE for events, telemetry, assertions, failure reports.
- HTTP 200 / 201 / 404 paths for every documented route.
- `POST /api/demo/run` end-to-end: persist, then `GET` the failure report and verify the reason.

## Out of scope

- No React UI.
- No MAVLink ingestion.
- No auth, no API keys, no users.
- No SSE / WebSocket yet (issue 3 will decide whether live updates are needed for the demo).
- No rate limiting.
- No pagination cursors beyond `limit`/`offset`.
- No write endpoints other than `POST /api/demo/run`.

## Notes for future issues

- **Issue 3** (UI) will consume these endpoints from the Vite dev server on port 5173. The CORS allowlist must include that origin. Keep DTO field names stable; the frontend types will mirror them.
- **Issue 4** (MAVLink) will introduce a long-lived ingestion task that writes runs/events/telemetry as data streams in. Repos should already support `insert` of individual events and samples; if a batched insert is added, keep the single-row API too.
- **Issue 4** may also introduce `GET /api/runs/:id/telemetry` and an SSE/WebSocket stream like `/api/runs/:id/stream`. Leave room in the router structure but do not implement them now.
- **Issue 5** (assertions) will write assertion results during ingestion or as a post-run pass. The `assertion_results` table is designed for both.
- Keep the JSON `payload` blob for normalized events forward-compatible (new variants must not break old rows). Always tag the variant with `kind` and parse `payload` lazily.

## Implementation notes

- **Workspace deps.** Added `sqlx 0.8` (with `runtime-tokio`, `tls-rustls`, `sqlite`, `macros`, `migrate`), `axum 0.7`, `tokio 1` (`rt-multi-thread`, `macros`, `net`, `signal`, `fs`), `tower 0.5`, `tower-http 0.5` (`trace`, `cors`), `http 1`, `http-body-util 0.1`, and `uuid` got the `v4` feature. The spec called for `runtime-tokio-rustls`; in sqlx 0.8 that feature is split into `runtime-tokio` + `tls-rustls`, so the Cargo entry uses the new shape.
- **Storage timestamp format.** RFC 3339 (from `time`) renders subseconds with variable width, so SQLite's lexicographic `ORDER BY at` reorders events with different precision (e.g. `…00.5Z` < `…00Z`). The storage layer writes a fixed-width nine-digit subsecond variant via a `time::format_description!` macro so column sort matches chronological order. Read paths still accept any RFC 3339 input.
- **Event payload split.** `normalized_events` stores the discriminator in a `kind` column and the variant-specific data (the rest of the serde-internally-tagged JSON object) in `payload`. On read the two are re-merged into `{"kind": <kind>, …payload}` and deserialized through `NormalizedEventKind`, so new variants only require a payload-shape change.
- **`RunBundle` lives in storage.** The storage crate defines its own `RunBundle` mirror of `flighttrace_demo::DemoRun` so storage does not depend on demo. The API handler builds a `RunBundle` from `generate_failed_takeoff_run()` and the demo crate has no SQL dependency.
- **`POST /api/demo/run` rewrites the demo bundle.** Every id (run, failure report, each event, each assertion) is regenerated with `Uuid::new_v4()`; the timeline is shifted by `now_utc() - bundle.started_at` so the persisted run is "fresh". An `EventId → EventId` map is threaded through `Evidence::Event` so internal references stay intact.
- **Enum JSON conventions.** `FailureReason` serializes as SCREAMING_SNAKE_CASE on the wire (`"TAKEOFF_TIMEOUT"`) per the spec. For consistency, `MissionStatus`, `MissionPhase`, `MissionSource`, and `AssertionOutcome` use the same casing in the DTO layer. Internally-tagged enums (`NormalizedEventKindDto`, `EvidenceDto`) keep PascalCase tag values to match the demo JSON.
- **`Evidence::Event` mapped to a tagged DTO.** The core `Evidence` is externally tagged; the DTO `EvidenceDto` is internally tagged with `kind` and exposes `eventId` as a camelCase field, so JS consumers see a uniform `{ "kind": "...", ... }` shape across evidence types.
- **CLI serve.** `--bind` defaults to `127.0.0.1:8080`; `--database` defaults to `./data/flighttrace.db`. The parent directory is created with `tokio::fs::create_dir_all`. Graceful shutdown is wired to Ctrl+C and SIGTERM via `tokio::signal`.
- **In-memory test pool.** `connect_in_memory` configures a single-connection SQLite pool against `sqlite::memory:` with `foreign_keys = ON`. API integration tests build the router directly against this pool and drive it with `tower::ServiceExt::oneshot`, so they need no socket or runtime spawn.

## Quality gates

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo test --workspace` — 36 tests pass across `flighttrace-core` (13), `flighttrace-demo` (11), `flighttrace-storage` (5), and `flighttrace-api` (7).
- Manual smoke: `flighttrace serve --database ./data/smoke.db` followed by `POST /api/demo/run`, `GET /api/runs`, `GET /api/runs/:id/failure-report`, and a process restart confirmed the run survives across restarts.
