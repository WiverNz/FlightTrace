# flighttrace-desktop

Tauri v2 desktop shell for FlightTrace. See
[`docs/issues/08-tauri-desktop-shell.md`](../../docs/issues/08-tauri-desktop-shell.md)
and
[`docs/issues/09-desktop-packaging-hardening.md`](../../docs/issues/09-desktop-packaging-hardening.md)
for the full spec, and the top-level [`README.md`](../../README.md#run-modes)
for run-mode context.

Not a workspace member — Tauri's Linux link-time deps (`webkit2gtk`,
GTK, glib, appindicator) are opt-in. Build from inside `src-tauri/`:

```bash
cd apps/flighttrace-desktop/src-tauri
cargo check            # requires the system deps below
cargo build --release  # same
cargo tauri dev        # requires `cargo install tauri-cli --version ^2`
cargo tauri build      # produces installers under target/release/bundle/
```

## What the shell does

The shell starts the same Axum router the CLI does — `flighttrace-api`
+ `flighttrace-storage` — on `127.0.0.1:0` (a free loopback port), then
points a Tauri WebView at that URL. No Tauri commands, no custom IPC,
no duplicated business logic. Closing the window flips a
`watch::channel` shutdown signal that Axum's graceful-shutdown future
picks up.

## Where things live at runtime

- **Web assets** (`web/dist`) — resolved in this order:
  1. `FLIGHTTRACE_WEB_DIR` env var.
  2. `<Tauri resource-dir>/web-dist` (bundled by `tauri.conf.json`
     `bundle.resources`).
  3. `<Tauri resource-dir>/_up_/web-dist` (macOS bundle layout).
  4. `<Tauri resource-dir>/web/dist` (fallback bundler layout).
  5. `<exe dir>/web-dist` (portable — just drop `web-dist/` next to the
     binary).
  6. `<repo>/web/dist` via `CARGO_MANIFEST_DIR` (works for
     `cargo tauri dev` from a checkout).

  If none of these have an `index.html`, the shell exits with a
  numbered list of every path it tried plus the recommended fix.

- **SQLite database** — priority:
  1. `FLIGHTTRACE_DATABASE` env var.
  2. `<AppHandle::path().app_data_dir()>/flighttrace-desktop.db` —
     `~/.local/share/com.flighttrace.desktop/` on Linux,
     `~/Library/Application Support/com.flighttrace.desktop/` on macOS,
     `%APPDATA%\com.flighttrace.desktop\` on Windows.
  3. `./data/flighttrace-desktop.db` — only used if `app_data_dir()`
     fails (rare).

## Linux system dependencies

On Ubuntu 24.04:

```bash
sudo apt install \
    libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev \
    libayatana-appindicator3-dev librsvg2-dev pkg-config \
    build-essential curl wget file libssl-dev
```

See the top-level README for Fedora/Arch equivalents and the WSL2 note
(`WEBKIT_DISABLE_DMABUF_RENDERER=1`).

## CI

The `desktop-check` job in `.github/workflows/ci.yml` runs
`cargo fmt --check` and `cargo check --locked` inside this crate on
`ubuntu-latest` with the apt packages above installed. It's independent
of the `rust` and `web` jobs — a red desktop check does *not* fail the
main workspace or web gates.
