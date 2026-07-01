# Issue 7 — Serve the prebuilt web UI from the Axum backend

## Summary

Make FlightTrace usable as one local process. After this issue,
`flighttrace serve --web-dir ./web/dist` starts the API *and* serves the
prebuilt React bundle from the same address, so a user can open
`http://127.0.0.1:8080` in a browser and get the full app — no Vite dev
server required.

The dev workflow (Vite on `:5173` + API on `:8080`) stays supported. When
`--web-dir` is omitted the backend behaves exactly as it did before this
issue.

No Tauri, no Electron, no Docker, no auth, no cloud. The single binary +
`web/dist` sitting next to it is the packaging story for v0.1.x.

## Product goal

After this issue, an evaluator can:

1. Clone the repo.
2. `cd web && npm ci && npm run build` once.
3. `cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db --web-dir ./web/dist`.
4. Open `http://127.0.0.1:8080` and use the app immediately.
5. `--open` flag optionally launches the default browser at that URL.

Existing quickstarts (dev mode, PX4/ArduPilot SITL) keep working with no
changes.

## Technical scope

- Static-file serving in `flighttrace-api` via `tower-http::services::ServeDir`,
  with SPA fallback to `index.html` for client-side routes like
  `/runs/<id>`.
- New CLI flags on `serve`:
  - `--web-dir <path>` (env `FLIGHTTRACE_WEB_DIR`)
  - `--open` (env `FLIGHTTRACE_OPEN`)
- `--web-dir` is validated at startup (directory exists, `index.html`
  readable). Fail fast on typos, same as `--mission`.
- API contract unchanged: `/health` and `/api/*` behave exactly as before,
  including 404 JSON envelopes for unknown API paths.
- Release workflow builds `web/dist` and includes it in the release
  archive next to the binary.
- No new runtime dependencies — the browser open helper is a small
  platform-conditional `std::process::Command` wrapper.

## Files and modules to create or modify

```
Cargo.toml                                        # tower-http "fs" feature
crates/flighttrace-api/src/router.rs              # build_with_static + /api/*rest 404
crates/flighttrace-api/src/lib.rs                 # re-export build_with_static
crates/flighttrace-api/tests/http.rs              # static-serving tests
crates/flighttrace-cli/src/main.rs                # --web-dir, --open, validation, browser
.env.example                                      # FLIGHTTRACE_WEB_DIR, FLIGHTTRACE_OPEN
README.md                                         # dev vs packaged run modes
.github/workflows/release.yml                     # build web/dist and package it
docs/issues/07-static-web-serving.md              # this file
```

## Detailed implementation steps

1. **`tower-http` gains the `fs` feature.** Update the workspace dep line
   in `Cargo.toml` to `features = ["trace", "cors", "fs"]`.

2. **Router changes** in `crates/flighttrace-api/src/router.rs`:
   - Add `pub fn build_with_static(state: AppState, web_dir: Option<PathBuf>) -> Router`.
   - Keep `pub fn build(state: AppState) -> Router` as a thin
     `build_with_static(state, None)` alias — every existing test and
     caller still compiles.
   - Add a `/api/*rest` catch-all bound to a small handler that returns
     the standard `not_found` JSON envelope. This prevents unknown
     `/api/...` paths from falling through to the SPA fallback and
     returning `index.html`.
   - When `web_dir` is `Some(dir)`, set the router's
     `fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html"))))`
     so unmatched non-`/api` paths resolve to a real file or, failing
     that, `index.html` (SPA fallback for `/runs/<id>` and friends).
   - CORS and tracing layers still wrap everything.

3. **CLI changes** in `crates/flighttrace-cli/src/main.rs`:
   - Add `--web-dir` (`Option<PathBuf>`, env `FLIGHTTRACE_WEB_DIR`).
   - Add `--open` (bool flag, env `FLIGHTTRACE_OPEN`).
   - Validate `--web-dir` up front: directory exists, `index.html` is a
     readable file. Fail with `anyhow::Context` if not — same pattern as
     the mission-YAML preflight in issue 4/5.
   - Pass the validated directory into `flighttrace_api::build_with_static`.
   - After `TcpListener::bind` succeeds, if `--open` is set spawn a
     background task that shells out to the platform's browser launcher
     (`xdg-open`, `open`, or `cmd /c start`). Errors are logged, never
     fatal — headless CI environments must still run happily.

4. **Tests** in `crates/flighttrace-api/tests/http.rs`:
   - Use a helper that creates a scratch `web-dir` with `index.html` and
     `assets/app.js` in `std::env::temp_dir()`. Random subdirectory per
     test for isolation; RAII guard removes on drop.
   - `health_still_works_with_web_dir` — `GET /health` returns the same
     `{"status":"ok","version":...}` JSON.
   - `api_still_works_with_web_dir` — `POST /api/demo/run` then
     `GET /api/runs` still work.
   - `root_serves_index_html` — `GET /` returns the marker bytes from
     `index.html`.
   - `spa_fallback_serves_index_html` — `GET /runs/some-id` returns the
     same `index.html` bytes (SPA fallback).
   - `assets_serve_actual_file` — `GET /assets/app.js` returns the JS
     bytes, not the HTML.
   - `unknown_api_path_still_returns_json_404` — `GET /api/nonsense`
     returns the standard `{"error":{"code":"not_found",…}}` envelope
     rather than falling through to `index.html`.

5. **`.env.example`** gains two lines:
   ```
   # Uncomment to serve the prebuilt web UI from the same origin.
   # FLIGHTTRACE_WEB_DIR=./web/dist
   # FLIGHTTRACE_OPEN=1
   ```

6. **README** gets a "Run modes" note in the Quickstart:
   - Dev mode (two terminals: `cargo run …` + `npm run dev`) — unchanged.
   - Packaged mode: `npm run build` once, then a single
     `flighttrace serve --web-dir ./web/dist` — open the API address in
     a browser.
   - Explain the `--open` convenience flag.

7. **Release workflow** in `.github/workflows/release.yml`:
   - Add Node.js setup + `web/` cache before the archive step.
   - Run `npm ci` and `npm run build` inside `web/`.
   - Copy `web/dist/**` next to the binary in each platform's archive
     tree (`dist/<package>/web-dist/`), and mention it in the
     archive-contents note in the release body.

8. **Quality gates:**
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cd web && npm run build && npm run lint`

## Acceptance

- [x] `cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db` still works exactly as before (no static serving, API returns 404 for unknown paths).
- [x] `cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db --web-dir ./web/dist` serves the built UI at `http://127.0.0.1:8080/` and still exposes `/health` and `/api/*`.
- [x] `curl -fsS http://127.0.0.1:8080/health` returns the JSON envelope.
- [x] `curl -fsSI http://127.0.0.1:8080/` returns 200 with `content-type: text/html`.
- [x] `curl -fsSI http://127.0.0.1:8080/runs/does-not-exist` returns 200 (SPA fallback).
- [x] `curl -fsSI http://127.0.0.1:8080/api/does-not-exist` returns 404 JSON.
- [x] `--open` launches the default browser on Linux/macOS/Windows and no-ops on unknown platforms.
- [x] Release archives contain `flighttrace` (or `flighttrace.exe`) and a `web-dist/` sibling folder.
- [x] All quality gates green locally.

## Implementation notes

- **Router**: `flighttrace_api::build_with_static(state, Option<PathBuf>)` is the new seam; `build(state)` is kept as a thin `build_with_static(state, None)` wrapper so every existing caller and test compiles unchanged.
- **API-only guard rail**: added `.route("/api/*rest", any(api_not_found))` so the SPA fallback never swallows unknown `/api/...` URLs. The catch-all returns `ApiError::NotFound`, giving the same `{"error":{"code":"not_found",...}}` envelope as before.
- **SPA fallback**: `ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))` + `router.fallback_service(...)`. Assets under the tree resolve to real files (correct MIME types); anything else resolves to `index.html`, so React Router owns client-side routes like `/runs/<id>`.
- **CLI**: added `--web-dir` (`FLIGHTTRACE_WEB_DIR`) and `--open` (`FLIGHTTRACE_OPEN`) with the same env-fallback pattern as the other flags. `validate_web_dir` runs up front — a missing directory or missing `index.html` fails fast before the listener binds. `open_url_in_browser` shells out to `xdg-open` / `open` / `cmd /c start` per target OS; errors are logged and never propagate.
- **Tower feature**: added `"fs"` to the workspace `tower-http` features for `ServeDir` / `ServeFile`.
- **Release workflow**: added `actions/setup-node@v4` + `npm ci && npm run build` before the archive step, and `cp -R web/dist "dist/$package/web-dist"` (PowerShell equivalent on Windows) so archives now ship `flighttrace` + `web-dist/`.
- **README**: added a "Run modes — dev vs. packaged" section describing the one-process workflow with `--web-dir` and `--open`, plus new `FLIGHTTRACE_WEB_DIR` / `FLIGHTTRACE_OPEN` rows in the env table.
- **Tests**: `crates/flighttrace-api/tests/http.rs` gained `ScratchDir` (a UUID-named temp directory with RAII cleanup so we don't add `tempfile` as a dev-dep), and six new integration tests exercising `/health`, `/api/*`, `/`, SPA fallback, real asset serving, and the `/api/*rest` 404. The CLI env-precedence suite now covers `--web-dir` / `--open` too.

## Verification

- `cargo fmt --all -- --check` ✅
- `cargo clippy --workspace --all-targets -- -D warnings` ✅
- `cargo test --workspace` ✅ — API tests 20/20, CLI tests 6/6, MAVLink tests 36/36, storage 5/5.
- `cd web && npm run build && npm run lint` ✅
- Smoke test against `flighttrace serve --bind 127.0.0.1:18990 --web-dir ./web/dist`:
  - `GET /health` → `{"status":"ok","version":"0.1.2"}`
  - `HEAD /` → `200 OK, content-type: text/html`
  - `HEAD /runs/does-not-exist` → `200 OK, content-type: text/html` (SPA fallback)
  - `GET /api/nonsense` → `404`, body `{"error":{"code":"not_found","message":"resource not found"}}`
  - `HEAD /assets/index-<hash>.js` → `200 OK, content-type: text/javascript` (real file, no SPA fallthrough)
  - `GET /api/runs` → `200 OK`, empty array (no demo runs on this smoke DB)

## Deviations

- Chose to hand-roll `open_url_in_browser` with `std::process::Command` rather than pull the `open` crate as a dep. Keeps the workspace dep list unchanged and covers Linux/macOS/Windows.
- Used a UUID-named `ScratchDir` in tests instead of adding `tempfile` as a dev-dep, for the same reason.
- `--open` is a plain `bool` flag with clap's `env` fallback. clap treats bare `--open` as `true` and there is no `--no-open` today; if a future case wants "env says true, CLI wants off," we'd need `Option<bool>` or a `--no-open` companion.
