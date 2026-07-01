# HTTP API

The Axum server in `flighttrace-api` exposes a small REST surface
under `/api/` plus a `/health` probe. All four run modes documented in
[`run-modes.md`](run-modes.md) speak the same contract.

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/health` | Liveness probe — `{"status":"ok","version":"…"}`. |
| GET | `/api/runs?limit&offset` | Mission runs, ordered by `startedAt DESC`. |
| GET | `/api/runs/:id` | Full run header (status, failure reason, phases). |
| GET | `/api/runs/:id/events` | Normalized events for the run, time-ordered. |
| GET | `/api/runs/:id/telemetry` | Telemetry samples for the run, time-ordered. |
| GET | `/api/runs/:id/assertions` | Assertion results for the run. |
| GET | `/api/runs/:id/failure-report` | Structured failure report; 404 if the run has none. |
| POST | `/api/runs/:id/reevaluate` | Re-runs the assertion engine. Body `{ "missionPath": "…" }` is optional. |
| POST | `/api/demo/run` | Generates the deterministic demo failed run and returns `{ "id": "…" }`. |

## Conventions

- **JSON only.** Request and response bodies are JSON. Requests that
  need a body must set `Content-Type: application/json`.
- **camelCase.** All fields are `camelCase` on the wire; Rust uses
  `snake_case` and the DTO layer in
  `crates/flighttrace-api/src/dto.rs` handles the conversion via
  `#[serde(rename_all = "camelCase")]`.
- **Internally-tagged enums.** `NormalizedEventKind` uses
  `{"kind": "…"}` as the discriminator (e.g.
  `{"kind":"ArmedChanged","armed":true}`). `FailureReason`,
  `MissionStatus`, `MissionPhase`, `MissionSource`, and
  `AssertionOutcome` serialize as `SCREAMING_SNAKE_CASE` strings.

## Error envelope

Errors return the following JSON body along with the documented HTTP
status:

```json
{
  "error": {
    "code": "not_found",
    "message": "…"
  }
}
```

Codes in v0.1: `not_found`, `bad_request`, `internal_error`.

Unknown `/api/*` paths return `404 not_found` (rather than falling
through to the SPA) — see the `/api/*rest` catch-all in
`crates/flighttrace-api/src/router.rs`.

## CORS

The dev CORS allowlist accepts `http://localhost:5173` and
`http://127.0.0.1:5173`. Packaged web mode and the desktop shell serve
the SPA on the same origin as the API, so CORS doesn't apply.

## Smoke test

The repo ships `scripts/release-smoke.sh` which validates a packaged
tree end-to-end:

```bash
scripts/release-smoke.sh                     # dev-local layout
scripts/release-smoke.sh --pkg-dir dist/foo  # explicit archive tree
```

It boots the binary against a prebuilt frontend on a random loopback
port, hits `/health`, `HEAD /`, `POST /api/demo/run`,
`GET /api/runs/<id>/failure-report`, `GET /api/nonsense`, and
`HEAD /runs/<id>` (SPA fallback), and tears the server down cleanly
on both success and failure.
