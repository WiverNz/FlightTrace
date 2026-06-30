# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Authoritative docs

Read these before opening anything else; they win over guessing from code:

- `docs/prd.md` — product scope (what we will and will not build for v0.1).
- `docs/context.md` — working conventions (vocabulary, exclusions, quality bar).
- `docs/issues/01..06-*.md` — per-issue history with implementation notes,
  verification logs, and deviations from spec for each landed feature.
- `docs/release-checklist.md` — what "done" means for a tagged release.

`docs/prd.md` wins on scope. `docs/context.md` wins on style.

## Commands

```bash
# Quality gates (same as CI in .github/workflows/ci.yml).
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm ci && npm run build && npm run lint

# Single test (substring match across workspace).
cargo test --workspace <name_or_substring>

# Single test in one crate, with stdout.
cargo test -p flighttrace-assertions <name> -- --nocapture

# Print the deterministic demo bundle as JSON (smoke test).
cargo run -p flighttrace-cli -- demo | head -n 40

# Start the API + SQLite, demo-only mode.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 --database ./data/flighttrace.db

# Same, plus read-only MAVLink ingestion + assertion engine on session close.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 --database ./data/flighttrace.db \
    --mavlink udp://127.0.0.1:14550 \
    --mission examples/missions/simple_takeoff.yaml

# Frontend dev server (talks to the API via VITE_API_BASE_URL in web/.env.development).
cd web && npm install && npm run dev
```

## Architecture invariants

The codebase is small and the crate boundaries are load-bearing. Several
invariants only become visible by reading multiple files — preserve them
when editing.

- **`flighttrace-core` is the bottom of the dependency graph.** All domain
  types (`MissionRun`, `NormalizedEvent`, `TelemetrySample`,
  `AssertionResult`, `FailureReport`, `Evidence`, the `FailureReason`
  enum) live there. Other crates depend on it, never the reverse. Don't
  re-define types in `flighttrace-storage`, `-api`, or `-assertions`.
- **Normalized events + telemetry samples are the only contract** between
  sources (demo, MAVLink) and consumers (storage, API, UI, assertion
  engine). Demo data and real data flow through the same pipeline — when
  changing event shape, both must keep working.
- **Read-only MAVLink is enforced by test.** `crates/flighttrace-mavlink`
  exposes no `send` function and an in-tree integration test
  (`tests/read_only_invariant.rs`) greps the crate for forbidden sends
  and checks the sentinel comment in `lib.rs` and `listener.rs`. Do not
  add a send path even "temporarily."
- **API JSON is `camelCase`; Rust is `snake_case`.** Conversion happens at
  the DTO layer in `crates/flighttrace-api/src/dto.rs` via
  `#[serde(rename_all = "camelCase")]`. Don't sprinkle rename attributes
  on core types.
- **Internally-tagged enums on the wire.** `NormalizedEventKind` is
  `#[serde(tag = "kind")]` (e.g. `{"kind":"ArmedChanged","armed":true}`).
  `FailureReason` and `MissionStatus`/`MissionPhase`/`MissionSource`/
  `AssertionOutcome` serialize as `SCREAMING_SNAKE_CASE` in the DTO layer
  (`{"failureReason": "TAKEOFF_TIMEOUT"}`).
- **Storage timestamps use fixed-width 9-digit subsecond RFC 3339** so
  SQLite's lexicographic `ORDER BY at` matches chronological order. Read
  paths still accept variable-width input. See
  `crates/flighttrace-storage/src/pool.rs` and the time format macro.
- **`evaluate(&MissionSpec, &EvaluationInput) -> EvaluationOutput` is
  pure.** It does no I/O. New checks go under
  `crates/flighttrace-assertions/src/checks/` as pure functions. The
  failing-check → `FailureReason` mapping lives in
  `crates/flighttrace-assertions/src/failure.rs`.
- **Assertion + FailureReport IDs are deterministic** via UUID v5 over a
  crate-local namespace (`flighttrace-assertions::ids`). This keeps the
  demo bundle's JSON byte-stable across calls — the
  `generator_is_deterministic` test catches regressions.
- **`POST /api/demo/run` rewrites every id** with `Uuid::new_v4()` and
  shifts the deterministic demo timeline so `started_at` becomes "now".
  An `EventId → EventId` map is threaded through `Evidence::Event` so
  cross-references stay intact. Don't break this when touching
  `crates/flighttrace-api/src/handlers/demo.rs`.
- **CLI parses the mission spec up-front.** `serve --mission <path>` loads
  + parses the YAML in `run_serve` before binding the listener, so typos
  fail fast. Keep that order.

## Vocabulary

Use these exact words in code, docs, UI, and commit messages — synonyms
get renamed before merge:

- `FlightTrace`, `mission run`, `mission timeline`, `normalized event`,
  `telemetry sample`, `assertion`, `failure report`, `evidence`,
  `SITL`, `MAVLink`, `PX4`, `ArduPilot`.

Banned synonyms include "flight"/"session" (use mission run), "metric"/
"datapoint" (use telemetry sample), "check"/"rule" (use assertion),
"incident"/"postmortem" (use failure report).

## Scope discipline

`docs/context.md` §7 lists what we will **not** introduce in v0.1:
Kubernetes, Kafka/NATS, ROS 2, TimescaleDB/ClickHouse, cloud services,
auth/SSO/RBAC, billing, multi-user, companion-computer agents, mission
upload / command sending, AI/LLM root-cause analysis. These exclusions are
non-negotiable for v0.1 — flag any user request that pulls in this
direction before implementing.

When in doubt about whether a change belongs in v0.1, the test is: does
this help answer "why did this mission run fail?" If not, cut it.

## Per-issue working pattern

Implementation is organized into sequential issues (`docs/issues/`). Each
issue file gets a Implementation notes / Verification / Deviations section
appended after the work lands, and its acceptance checkboxes flipped from
`[ ]` to `[x]`. Mirror this pattern for new issues — `05` and `06` are good
templates.

A landed issue means: acceptance criteria ticked **and** all quality gates
green **and** behaviour-changing docs updated. `cargo run -p flighttrace-cli
-- demo | head -n 40` is the cheapest smoke test that the core pipeline is
still coherent.
