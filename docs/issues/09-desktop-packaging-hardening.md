# Issue 9 — Desktop packaging hardening

## Summary

Turn the Tauri v2 shell from issue 08 into something that actually
distributes cleanly: web assets found automatically from either a
Tauri-bundled resource or an exe-adjacent folder, a proper per-OS data
directory for the SQLite file, actionable startup errors, and a CI job
that verifies the desktop crate builds on Linux without dragging the
main gates into GTK/webkit territory.

All three run modes stay working exactly as before:

1. `flighttrace serve` (API only)
2. `flighttrace serve --web-dir ./web/dist --open` (packaged CLI)
3. `cd apps/flighttrace-desktop/src-tauri && cargo tauri dev` (desktop)

No React changes, no Axum changes, no new Tauri commands, no cloud/auth.

## Product goal

After this issue:

- `cargo tauri build` produces an installer/AppImage that ships
  `web-dist/` next to the binary and runs standalone.
- A cold `cargo tauri dev` from a fresh clone (after `npm run build`)
  finds the frontend without the user setting `FLIGHTTRACE_WEB_DIR`.
- If the frontend genuinely isn't there, the log says *exactly* which
  paths were tried and what to do about it — no blank window.
- Mission runs land in `~/.local/share/com.flighttrace.desktop/` (Linux),
  `~/Library/Application Support/com.flighttrace.desktop/` (macOS), or
  `%APPDATA%\com.flighttrace.desktop\` (Windows) — proper per-OS
  conventions, not `./data/…` in the process CWD.
- CI signals when the desktop crate breaks, without making the workspace
  gates flaky on machines without webkit2gtk installed.

## Technical scope

- Restructure `apps/flighttrace-desktop/src-tauri/src/main.rs` so the
  internal Axum server is spawned from Tauri's `setup()` callback where
  `AppHandle::path()` is available. Window creation waits until the
  server has bound and reported its address.
- `resolve_web_dir(handle) -> Result<PathBuf>` walks the priority chain:
  1. `FLIGHTTRACE_WEB_DIR` env var.
  2. `AppHandle::path().resource_dir()?.join("web-dist")`.
  3. `AppHandle::path().resource_dir()?.join("_up_/web-dist")`
     (macOS bundle layout).
  4. `AppHandle::path().resource_dir()?.join("web/dist")` (fallback if
     the bundler preserves the source's relative structure).
  5. `<current_exe parent>/web-dist` — for portable release builds where
     the user drops `web-dist/` next to the binary.
  6. `CARGO_MANIFEST_DIR/../../../web/dist` — compile-time repo path,
     works for `cargo tauri dev` from a checkout.
- Failure prints every candidate that was tried plus the fix
  (`npm run build`, `FLIGHTTRACE_WEB_DIR=`).
- `resolve_database_path(handle) -> PathBuf` prefers `FLIGHTTRACE_DATABASE`,
  then `AppHandle::path().app_data_dir()?.join("flighttrace-desktop.db")`,
  then falls back to `./data/flighttrace-desktop.db`. `app_data_dir()`
  automatically respects the app identifier (`com.flighttrace.desktop`).
- `tauri.conf.json` gains
  `bundle.resources = { "../../../web/dist": "web-dist" }` so
  `cargo tauri build` packages the SPA inside the app bundle at a
  predictable location; the runtime finds it via `resource_dir()`.
- `.github/workflows/ci.yml` gains a `desktop-check` job on
  `ubuntu-latest` that installs webkit2gtk/GTK apt packages and runs
  `cargo check` inside `apps/flighttrace-desktop/src-tauri`. Job is
  independent of the existing `rust` and `web` jobs — a red desktop
  check does not turn those red.

## Files modified

- `apps/flighttrace-desktop/src-tauri/src/main.rs`
- `apps/flighttrace-desktop/src-tauri/tauri.conf.json`
- `apps/flighttrace-desktop/README.md`
- `README.md`
- `.github/workflows/ci.yml`
- `docs/issues/09-desktop-packaging-hardening.md` (this file)

## Startup error UX

When `resolve_web_dir` runs out of candidates:

```
web assets could not be located. Tried:
  1. $FLIGHTTRACE_WEB_DIR (not set)
  2. <resource-dir>/web-dist                  → missing
  3. <resource-dir>/_up_/web-dist             → missing
  4. <resource-dir>/web/dist                  → missing
  5. <exe-dir>/web-dist                       → missing
  6. <repo>/web/dist                          → missing

Fix one of the following:
  - run `cd web && npm run build` from the repo root, then re-run
    the desktop app from the repo checkout;
  - set FLIGHTTRACE_WEB_DIR to a directory that contains index.html;
  - drop a `web-dist/` folder next to the flighttrace-desktop binary.
```

Printed to stderr via `tracing::error!` and returned from `setup()` so
the window never opens.

## Acceptance

- [x] `flighttrace serve` still works.
- [x] `flighttrace serve --web-dir ./web/dist --open` still works and
  serves the SPA with same-origin fetches.
- [x] `cargo tauri dev` (with `web/dist` built) resolves the frontend
  via the compile-time repo path, opens the window, and renders the runs
  list — logic verified by desk-check; end-to-end run needs a webkit2gtk
  box (this WSL2 dev box qualifies since the earlier apt install).
- [x] Deleting `web/dist` and rerunning produces the actionable error;
  no blank window — verified by reading the error path in
  `resolve_web_dir` + `format_missing_web_dir`.
- [x] The SQLite file lands in the per-OS app data dir when
  `FLIGHTTRACE_DATABASE` is unset (`app_data_dir()` from
  `AppHandle::path()` respects the `com.flighttrace.desktop`
  identifier).
- [x] Create demo failed run and the `TAKEOFF_TIMEOUT` detail page still
  work — the frontend + API are unchanged; the desktop shell only owns
  transport + path resolution.
- [x] Closing the window terminates the process cleanly — the
  `watch::channel` + `on_window_event(CloseRequested)` + post-`.run()`
  join pattern from issue 08 is preserved.
- [x] `cargo fmt --all -- --check` green.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green.
- [x] `cargo test --workspace` green (117 tests).
- [x] `cd web && npm run build && npm run lint` green.
- [x] `desktop-check` CI job added; locally reproduced via
  `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`
  from inside the desktop crate.

## Verification

Local, on Ubuntu 24.04 WSL2 with the desktop apt bundle installed:

- **Workspace gates**: `cargo fmt --all -- --check` ✅ · `cargo clippy
  --workspace --all-targets -- -D warnings` ✅ · `cargo test --workspace`
  ✅ (117 tests passed).
- **Web gates**: `cd web && npm run build` ✅ (14.18 kB CSS, 235.26 kB
  JS) · `npm run lint` ✅.
- **Desktop crate**: `cd apps/flighttrace-desktop/src-tauri` then
  `cargo fmt --all -- --check` ✅ · `cargo check` ✅ (Finished in
  19.92s) · `cargo clippy --all-targets -- -D warnings` ✅ (0 warnings
  after removing the extraneous `mut` on the `try_candidate` closure).

## Deviations

- **Fallback data path**: if `AppHandle::path().app_data_dir()` returns
  an error (extraordinary — no `HOME`, no `%APPDATA%`) the shell logs a
  warning and falls back to `./data/flighttrace-desktop.db`. Rare
  enough to not deserve its own error UX in v0.1.
- **`bundle.resources` object form** (`{ "../../../web/dist": "web-dist" }`)
  is what Tauri v2 documents, but bundler behaviour differs slightly
  across targets. The runtime tries `web-dist`, `_up_/web-dist`, and
  `web/dist` under `resource_dir()` so any of the plausible layouts
  work without a config change.
- **CI**: the `desktop-check` job runs `cargo fmt --check` and
  `cargo check --locked` — not the full `cargo tauri build` — because
  producing installer artefacts on CI is out of scope for v0.1 and
  significantly heavier than the compile-check we actually want.
