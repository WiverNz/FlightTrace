# FlightTrace — Working Context

This document is the short, opinionated "how we build FlightTrace" companion to `prd.md`. The PRD says **what** we are building. This file says **how we work, what we believe, and what we refuse to do.**

Read this before opening an issue file.

## 1. What FlightTrace is, in one sentence

> A developer-first mission debugger that explains **why** autonomous PX4/ArduPilot missions fail, with evidence.

If a feature does not help answer "why did this run fail?", it does not belong in v0.1.

## 2. What FlightTrace is not

- Not a telemetry dashboard.
- Not an AirData / Flight Review clone.
- Not a ground control station.
- Not a fleet management product.
- Not a generic time-series store.
- Not a cloud product.
- Not multi-user.
- Not an AI auto-diagnosis tool.

When in doubt, cut features that drift toward these categories.

## 3. Target user

A working PX4 / ArduPilot / robotics developer. Comfortable with the terminal, YAML, MAVLink terms, and reading structured data. They want **information density** and **truth**, not a friendly empty state with a unicorn.

Design implication: the UI looks like a debugger. Monospace, dense tables, timeline, evidence cards, raw event log. Not a marketing dashboard.

## 4. Core workflow (memorize this)

1. Start backend.
2. Start frontend.
3. Click **"Create demo failed run"**.
4. See a `TAKEOFF_TIMEOUT` failed run in the list.
5. Open it. See timeline, telemetry, assertions, evidence, raw events.
6. Understand why it failed without reading source code.

Everything in v0.1 exists to make this loop true and tight.

## 5. Architecture principles

- **Local-first.** SQLite file on disk. No external services.
- **Read-only on real vehicles.** FlightTrace ingests MAVLink. It does not send commands. Ever, in v0.1.
- **Normalized events are the contract.** Everything (UI, assertions, failure reports) consumes normalized events and telemetry samples. MAVLink and the demo generator are *sources*. Sources are interchangeable.
- **Demo data and real data go through the same pipeline.** Same domain types, same storage, same assertion engine. The demo is not a mock; it is a deterministic source.
- **Small crates, clear boundaries.** Each crate in `crates/` has one responsibility and one public surface.
- **One source of truth for the domain.** `flighttrace-core` owns the types. Nothing else redefines them.

## 6. Tech choices (and why)

- **Rust + Tokio + Axum**: fast, type-safe, easy to embed MAVLink parsing, easy to ship a single binary later.
- **SQLite + sqlx**: zero-ops local storage. We do not need Postgres in v0.1. If we ever do, sqlx makes the move tractable.
- **serde / thiserror / anyhow / tracing**: standard, boring, correct.
- **React + TypeScript + Vite**: fast dev loop, no SSR complexity, easy to host alongside the API.
- **rust-mavlink** (or current best-maintained crate): isolated behind `flighttrace-mavlink`.
- **No GraphQL, no gRPC, no tRPC.** Plain REST + SSE.
- **No state library cargo-culting on the frontend.** Start with `fetch` and React state. Add a library only if a real need appears.

## 7. What we explicitly will not introduce in v0.1

- Kubernetes
- Kafka / NATS / message brokers
- ROS 2
- TimescaleDB / ClickHouse / any external time-series DB
- Cloud services (S3, RDS, hosted Postgres, hosted Redis)
- Auth / SSO / RBAC
- Billing / subscription / accounts
- Multi-user / multi-tenant
- Companion-computer agents
- Mission upload / command sending
- AI / LLM root cause analysis

These exclusions are not negotiable for v0.1.

## 8. Repository layout

```
flighttrace/
  Cargo.toml                          # workspace manifest
  crates/
    flighttrace-core/                 # domain types, ids, errors, time
    flighttrace-demo/                 # deterministic failed run generator
    flighttrace-storage/              # sqlx + SQLite repos + migrations
    flighttrace-api/                  # Axum REST + SSE
    flighttrace-mavlink/              # MAVLink UDP ingestion (issue 4)
    flighttrace-assertions/           # YAML assertions + failure reports
    flighttrace-cli/                  # `flighttrace` binary
  web/                                # React + TS + Vite
    package.json
    src/
      pages/
      components/
      api/
      types/
  examples/
    missions/
      simple_takeoff.yaml
  demo-runs/                          # optional fixtures
  docs/
    prd.md
    context.md
    issues/
  README.md
  docker-compose.yml                  # optional, later
```

## 9. Naming and vocabulary (use these exact words)

In code, docs, UI, commit messages, and conversation:

- `FlightTrace`
- `mission run` (not "flight", not "session")
- `mission timeline`
- `normalized event` (not "event log entry", not "message")
- `telemetry sample` (not "metric", not "datapoint")
- `assertion` (not "check", not "rule")
- `failure report` (not "incident", not "postmortem")
- `evidence` (not "reason list", not "facts")
- `SITL`, `MAVLink`, `PX4`, `ArduPilot` (exact casing)

If a PR introduces a synonym, rename it before merging.

## 10. Quality bar per issue

Before an issue is considered done:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] If the issue touches the frontend: `npm run build` and `npm run lint` pass
- [ ] All acceptance criteria checkboxes in the issue file are ticked
- [ ] README and `docs/` are updated if behavior changed
- [ ] No new dependency on a banned tech (see section 7)

## 11. Coding conventions (short version)

- Prefer `thiserror` for library errors, `anyhow` for application-level glue.
- All domain types live in `flighttrace-core`. Other crates depend on it, never the reverse.
- IDs are UUIDs (v4 or v7), serialized as strings in JSON.
- Timestamps are UTC, stored as RFC 3339 strings in SQLite, as `OffsetDateTime` / `DateTime<Utc>` in Rust, as ISO strings on the wire.
- API JSON uses `camelCase` field names. Rust uses `snake_case`. `serde(rename_all = "camelCase")` at the boundary.
- Tracing spans wrap meaningful operations (HTTP requests, ingestion sessions, assertion runs).
- No `println!` outside CLI tooling — use `tracing`.

## 12. When this document and the PRD disagree

The PRD wins on scope. This document wins on style and conventions. If a real conflict appears, fix it in a PR — do not let both drift.
