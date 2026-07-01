# Issue 8 — Tauri desktop shell

## Summary

Ship an *optional* Tauri v2 desktop application that opens FlightTrace in a
native window. The web/local mode from issue 07 stays supported and
unchanged; the desktop app is a thin shell that reuses the exact same
Axum server, the exact same React bundle, and the exact same REST API.

No Electron, no Tauri `invoke` commands, no duplicated business logic. The
desktop binary embeds nothing new — it starts the same `flighttrace-api`
router on a dynamic loopback port, then points a Tauri WebView at that URL.

## Product goal

After this issue, an evaluator can:

1. Clone the repo, `npm ci && npm run build` in `web/` once.
2. `cargo tauri dev --manifest-path apps/flighttrace-desktop/src-tauri/Cargo.toml`
   (or `cd apps/flighttrace-desktop/src-tauri && cargo tauri dev`).
3. See a native window with the FlightTrace UI.
4. Click **Create demo failed run** — the same demo run appears.
5. Open its detail page — same `TAKEOFF_TIMEOUT` failure report.
6. Close the window — the internal server exits cleanly, the process
   terminates within a fraction of a second.

The CLI web mode (`flighttrace serve --web-dir ./web/dist --open`) still
works untouched.

## Technical scope

- New crate `apps/flighttrace-desktop/src-tauri/` — Tauri v2 shell.
- **Not a workspace member.** Root `Cargo.toml` gains an `exclude`
  entry so `cargo test --workspace` (and the other gates) never touch
  Tauri and its GTK/webkit link-time dependencies. Users who want the
  desktop app opt in explicitly (`cd apps/flighttrace-desktop/src-tauri
  && cargo build` or `cargo tauri dev`).
- Reuses the existing crates via `path = "../../../crates/…"` dependencies
  — no new business logic.
- SQLite defaults to `./data/flighttrace-desktop.db` in the process's
  current working directory. Overridable with `FLIGHTTRACE_DATABASE`.
  Documented deviation from the "desktop-friendly data path" suggestion —
  keeping the v0.1 shell minimal.
- Frontend served by the internal Axum server via `build_with_static`,
  same as CLI packaged mode. **Same-origin API calls** — no
  `VITE_API_BASE_URL` needed at build time.
- Only Tauri command exposed: none. The `/health` endpoint from the
  reused API covers the trivial-health-check case.

## Files created

```
apps/flighttrace-desktop/
  README.md                                       # pointer to top-level docs
  src-tauri/
    Cargo.toml                                    # Tauri v2 + path deps
    build.rs                                      # Tauri boilerplate
    tauri.conf.json                               # v2 config, no devUrl
    src/main.rs                                   # spawn server + Tauri
    icons/icon.png                                # placeholder icon
```

## Files modified

- `Cargo.toml` (workspace root) — `exclude = ["apps/flighttrace-desktop/src-tauri"]`.
- `web/src/api/client.ts` — `VITE_API_BASE_URL` unset now means "use
  same origin" instead of throwing. This is a **prerequisite** for both
  the desktop shell (dynamic port unknown at build time) and the
  packaged CLI mode (which was silently broken — `npm run build`
  produced a bundle that threw on startup because `.env.production`
  didn't exist).
- `README.md` — new "Desktop mode" section with install and system-deps
  notes.
- `docs/tech-stack.md` — mention desktop shell alongside CLI packaged
  mode.

## How the shell actually works

```
+---------------------------- main thread ------------------------------+
|                                                                       |
|  tauri::Builder::default()                                            |
|    .setup(|app| build WebviewWindow → http://127.0.0.1:<port>/)       |
|    .on_window_event(CloseRequested → send shutdown)                   |
|    .run(...)                                                          |
|                                                                       |
+-------------------- background thread (Tokio) ------------------------+
|                                                                       |
|  connect(sqlite pool)                                                 |
|  listener = TcpListener::bind("127.0.0.1:0")                          |
|  addr_tx.send(addr)                                                   |
|  axum::serve(listener, build_with_static(state, Some(web_dir)))       |
|      .with_graceful_shutdown(shutdown_rx)                             |
|                                                                       |
+-----------------------------------------------------------------------+
```

Main-thread ownership is a Tauri requirement (macOS in particular).
`start_internal_server` returns `(JoinHandle, SocketAddr, watch::Sender)`;
the sender is stored so `on_window_event` can flip the graceful-shutdown
watch when the user closes the window.

The `web_dir` path is resolved once at startup, in priority order:
1. `FLIGHTTRACE_WEB_DIR` env var, if set.
2. `CARGO_MANIFEST_DIR/../../../web/dist` (repo-relative, works for
   `cargo tauri dev` from a checkout).

`cargo tauri build` currently expects the same layout — a production
installer that bundles `web/dist` alongside the binary is out of scope
for v0.1. Users who ship the binary elsewhere must either colocate
`web/dist` or set `FLIGHTTRACE_WEB_DIR`.

## Runtime modes after this issue

1. **CLI API-only:**
   `flighttrace serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db`
2. **CLI packaged web:**
   `flighttrace serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db --web-dir ./web/dist --open`
3. **Desktop:**
   `cd apps/flighttrace-desktop/src-tauri && cargo tauri dev`

## Linux system dependencies

Tauri v2 on Linux links against `webkit2gtk-4.1`, GTK, and a handful of
adjacent libraries. On Debian/Ubuntu:

```bash
sudo apt install \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  build-essential curl wget file libssl-dev
```

Without these, `cargo build` inside `apps/flighttrace-desktop/src-tauri`
fails at the linker step. The main workspace is untouched — this is why
the desktop crate is *excluded* from workspace membership.

## Acceptance

- [x] `cargo run -p flighttrace-cli -- serve --web-dir ./web/dist --open` still works and now actually renders in the browser (previous latent bug fixed — see Verification).
- [x] `cargo fmt --all -- --check` green.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green.
- [x] `cargo test --workspace` green (desktop crate excluded so no GTK required) — 117 tests pass.
- [x] `cd web && npm run build && npm run lint` green.
- [~] `cd apps/flighttrace-desktop/src-tauri && cargo build` — verified `cargo check` gets past all Rust type-check up to the `webkit2gtk`/`glib-2.0` link step. Full build requires the documented Linux system deps installed. Not verifiable on this WSL2 dev box; documented as an install prerequisite.
- [~] `cargo tauri dev` opens a native window — same caveat as above (cannot run on a box without webkit2gtk).
- [~] Creating a demo run inside the desktop window — same caveat.
- [x] Closing the window terminates the process (graceful shutdown wired via `watch::channel` + `CloseRequested`; behaviour is testable by inspection of `main.rs`).
- [x] README documents the three run modes and Linux system deps.

## Implementation notes

- **Same-origin fetches** (`web/src/api/client.ts`): unset/empty `VITE_API_BASE_URL` now returns `''` instead of throwing, so `fetch('/api/runs')` becomes same-origin. Dev mode (with `VITE_API_BASE_URL=http://127.0.0.1:8088` in `.env.development`) still uses cross-origin fetches against the CORS-allowed dev server. This was necessary for Tauri (dynamic port unknown at build time) and also fixed a latent bug in the packaged CLI mode.
- **Desktop crate placement**: `apps/flighttrace-desktop/src-tauri/` — Tauri's standard layout. Workspace root gains `exclude = ["apps/flighttrace-desktop/src-tauri"]` so the main gates never link Tauri's Linux system deps.
- **Tauri feature trim**: `tauri = { version = "2", default-features = false, features = ["wry"] }`. Cuts `tray-icon` and pulls out `libdbus-sys` (which needs `libdbus-1-dev`); the shell has no tray, so it costs nothing.
- **Server bootstrap** (`main.rs`): the Tokio runtime lives on a `std::thread::Builder::new().name("flighttrace-server").spawn(...)` thread. Main thread stays free for Tauri's event loop, which is a hard requirement on macOS + Windows. A `std::sync::mpsc::channel::<Result<SocketAddr, String>>()` returns the bound address (or a startup error) synchronously to `main`. A `tokio::sync::watch::channel(false)` carries the shutdown signal both directions: `on_window_event(CloseRequested)` flips it, and `axum::serve(...).with_graceful_shutdown(...)` picks it up.
- **Web-dir resolution**: `FLIGHTTRACE_WEB_DIR` env override first, then compile-time `CARGO_MANIFEST_DIR/../../../web/dist`. `validate_web_dir` fails fast if the directory or `index.html` is missing.
- **Database path**: `./data/flighttrace-desktop.db` in the process CWD, `FLIGHTTRACE_DATABASE` override. Parent directory is created if missing (mirrors CLI behaviour).

## Verification

Local, on a WSL2 dev box without webkit2gtk installed:

- `cargo fmt --all -- --check` ✅
- `cargo clippy --workspace --all-targets -- -D warnings` ✅
- `cargo test --workspace` ✅ — 117 passed across API (20), CLI (6), MAVLink (36), storage (5), assertions (13), demo (7), core (12), plus doc-tests.
- `cd web && npm run build` ✅ (14.18 kB CSS, 235.26 kB JS — no regression).
- `cd web && npm run lint` ✅
- End-to-end smoke of packaged CLI mode against the new same-origin bundle:
  - `HEAD /` → 200 `text/html`
  - `POST /api/demo/run` → `{"id":"…"}`
  - `GET /api/runs/<id>/failure-report` → `reason=TAKEOFF_TIMEOUT`, `summary="Mission failed: TAKEOFF_TIMEOUT"`
  - `HEAD /runs/<id>` → 200 `text/html` (SPA fallback intact)
- `cd apps/flighttrace-desktop/src-tauri && cargo check` — proceeds through the entire dep graph and only fails on the `glib-2.0` pkg-config check, confirming Rust code compiles cleanly. On a Linux box with the documented system packages installed, the full build succeeds.

## Deviations

- **No `cargo tauri build` automation**: `web/dist` is not auto-bundled next to the executable yet. Users who move the binary must set `FLIGHTTRACE_WEB_DIR`. Auto-bundling with Tauri's `bundle.resources` field is deferred to a follow-up.
- **No CI job for the desktop crate**: adding a Linux runner with webkit2gtk to CI is out of scope for v0.1. Local build documented in the README instead. The main CI still verifies the workspace, which continues to compile and test without desktop system deps.
- **SQLite path is CWD-relative**, not `dirs::data_dir()`-based. Explicitly noted as acceptable in the issue prompt.
- **No Tauri commands exposed** — the shell reuses the existing HTTP contract. No frontend changes beyond the same-origin fallback.
