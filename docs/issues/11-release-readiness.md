# Issue 11 — Release readiness and screenshots

## Summary

Prepare FlightTrace for a presentable v0.1.0 GitHub release. No new
product features, no API changes, no new run modes. This issue is
about polish: the first screen of the README, a screenshot capture
guide + placeholders, a smoke-test script for release archives, a
ready-to-paste release note, and a clean bill of health across every
gate.

## Product goal

- A visitor lands on the GitHub repo and understands what FlightTrace
  is in under 10 seconds.
- They see the three run modes near the top: CLI/API-only, packaged
  local web, desktop.
- The demo path is unambiguous — five copy-paste commands from clone to
  seeing `TAKEOFF_TIMEOUT`.
- The read-only MAVLink guarantee is visible above the fold, not
  buried three sections deep.
- The "not a GCS / not AirData / not fleet-management" boundary is
  clear on the first screen.
- Release notes exist as a file that can be pasted into the GitHub
  Releases editor with no further edits.

## Technical scope

1. **README.md polish** — restructure the first ~40 lines:
   - One-line pitch matching the spec exactly.
   - Three-mode bullet block above the fold.
   - Safety one-liner up top; link to the deeper §Read-only guarantee.
   - Move "What it is not" bullets closer to the top so the boundary is
     visible immediately.

2. **`docs/screenshots/`** — a capture guide and four placeholder PNGs:
   - `README.md` — capture instructions (window size, dark theme, exact
     scenario per screenshot).
   - `runs-list.png`, `run-detail-failure-summary.png`,
     `run-detail-telemetry.png`, `desktop-window.png` — 800×500
     placeholder PNGs with palette-matching backgrounds and a
     `PLACEHOLDER` watermark so the README references never 404 and
     replacing them is a straight file-swap.

3. **`scripts/release-smoke.sh`** — a bash script that validates a
   packaged tree end-to-end:
   - Args: `--pkg-dir <dir>` (defaults to `dist/` beneath the repo root,
     falls back to the local `target/release` + `web/dist` combo).
   - Verifies the binary and `web-dist/index.html` exist.
   - Binds a random localhost port, starts `flighttrace serve
     --web-dir …`, waits for `/health`, runs:
     - `HEAD /` → 200 `text/html`
     - `POST /api/demo/run` → 201 with `{"id":"…"}`
     - `GET /api/runs/<id>/failure-report` → contains `TAKEOFF_TIMEOUT`
     - `HEAD /api/nonsense` → 404 with `content-type: application/json`
   - Tears down cleanly on success/failure via a `trap`.

4. **`docs/release-notes/v0.1.0.md`** — a copy-paste release body:
   - What FlightTrace is.
   - What works in v0.1.
   - How to run (three modes).
   - Limitations and known gaps.
   - Safety / read-only guarantee.

5. **Desktop verification** stays manual:
   - Existing `desktop-check` CI job (added in issue 09) covers
     `cargo fmt --check` + `cargo check --locked`.
   - `cargo tauri build` is documented in
     `apps/flighttrace-desktop/README.md` but not part of CI (heavy,
     out of v0.1.0 scope).

## Acceptance

- [x] README first screen names the product with the spec pitch,
  surfaces the three run modes in a table above the fold, calls out
  the read-only safety guarantee with a link to the deeper section,
  and lists the "not a GCS / not AirData / not fleet management"
  boundary in the first `## What FlightTrace is not` block.
- [x] `docs/screenshots/` has a capture guide (`README.md`) and four
  palette-matched placeholder PNGs (~12 KB each) wired into the main
  README's `## Screenshots` section.
- [x] `scripts/release-smoke.sh` runs against a local
  `target/release + web/dist` and exits 0 — verified below.
- [x] `docs/release-notes/v0.1.0.md` exists and can be pasted into the
  GitHub Releases editor without further edits.
- [x] `cargo fmt --all -- --check` green.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green.
- [x] `cargo test --workspace` green (19 `test result: ok` lines,
  matching baseline).
- [x] `cd web && npm run build && npm run lint` green (14.26 kB CSS,
  235.37 kB JS).
- [x] `cd apps/flighttrace-desktop/src-tauri && cargo check` green.
- [x] `cd apps/flighttrace-desktop/src-tauri && cargo clippy --all-targets -- -D warnings` green.

## Verification

- Workspace + web + desktop gates all green (see above).
- Release smoke against a fresh `cargo build --release -p flighttrace-cli`:
  ```
  → checking layout
    binary:   target/release/flighttrace
    web-dist: web/dist
  → starting server on 127.0.0.1:43621
  → waiting for /health
  → running checks
    OK   /health returns JSON status=ok
    OK   HEAD / returns 200 text/html
    OK   POST /api/demo/run → id=…
    OK   GET /api/runs/<id>/failure-report → TAKEOFF_TIMEOUT
    OK   GET /api/nonsense → 404 JSON envelope
    OK   HEAD /runs/<id> → SPA fallback to index.html
  release-smoke: all checks passed
  ```
- Placeholder generator `scripts/generate_screenshot_placeholders.py`
  is byte-idempotent (repeat runs produce identical files) and the
  four PNGs open cleanly (`file` reports valid 800×500 RGBA PNGs).

## Deviations

- **Screenshots are placeholders**, not real captures. Swapping in
  actual PNGs is a straight file-replace under `docs/screenshots/` —
  the recipe is in that directory's README.
- **`cargo tauri build` remains out of CI.** The `desktop-check` job
  from issue 09 (fmt-check + `cargo check --locked`) is the CI-side
  guarantee for the desktop crate; installer production is manual, as
  documented in `apps/flighttrace-desktop/README.md`.
- **No `.env.production`** ships. The client's same-origin fallback
  from issue 08 makes both packaged CLI mode and desktop mode work
  without one, and dev mode continues to use `.env.development`.
