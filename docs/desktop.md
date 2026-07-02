# Desktop mode (Tauri v2)

FlightTrace ships an optional Tauri v2 desktop shell that opens the
same UI in a native window. The shell is a **thin wrapper**: it starts
the same Axum server the CLI does on a random loopback port, then
points a WebView at that URL. Same API contract, same React bundle,
same SQLite schema. See [`run-modes.md`](run-modes.md) for how it
compares to the other modes.

## Downloaded Windows installer (end-user path)

The simplest way to run the native desktop app is to download the
Windows installer from the [GitHub Release](https://github.com/WiverNz/FlightTrace/releases)
page. Since v0.1.0 the release includes:

- **`flighttrace-desktop-<v>-x86_64-pc-windows-msvc-setup.exe`** —
  NSIS setup, ~10 MB. Recommended default.
- **`flighttrace-desktop-<v>-x86_64-pc-windows-msvc.msi`** — MSI
  variant for group-policy / enterprise deployment. Same content.

**WebView2 runtime**: preinstalled on Windows 11. On Windows 10 the
installer's NSIS bootstrapper fetches the runtime from Microsoft on
first launch. No manual install needed.

After install, launch **FlightTrace** from the Start menu. The app
opens a native window, no browser required. The internal Axum server
runs on a random loopback port and shuts down cleanly when you close
the window.

If you'd rather use your existing browser, download the CLI archive
instead — see [§ Run modes](../README.md#run-modes) for the two
side-by-side choices.

## Prerequisites

- **Node.js ≥ 18** to build `web/dist`.
- **`tauri-cli` v2** (`cargo install tauri-cli --version '^2'`) for
  `cargo tauri dev` and `cargo tauri build`.
- **Linux system packages** — the WebView links `webkit2gtk` etc. at
  build time. See below for the per-distro one-liners.
- **macOS / Windows** — no extra packages beyond the standard Tauri
  prerequisites documented at
  <https://v2.tauri.app/start/prerequisites/>.

The desktop crate is intentionally **not** a workspace member so
`cargo test --workspace` and CI remain independent of these link-time
deps.

## Debian / Ubuntu

```bash
sudo apt install \
    libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev \
    libayatana-appindicator3-dev librsvg2-dev pkg-config \
    build-essential curl wget file libssl-dev
```

On Ubuntu 24.04 LTS the correct package is `libwebkit2gtk-4.1-dev` —
`libwebkit2gtk-4.0-dev` is now a transitional dummy. Tauri v2
auto-links against `4.1`, so the command above is enough. If
`pkg-config` complains about `gobject-2.0` or `glib-2.0`,
`libglib2.0-dev` is the missing piece.

## Fedora / RHEL

```bash
sudo dnf install \
    webkit2gtk4.1-devel gtk3-devel glib2-devel \
    libappindicator-gtk3-devel librsvg2-devel \
    openssl-devel curl wget file
sudo dnf group install "C Development Tools and Libraries"
```

## Arch / Manjaro

```bash
sudo pacman -S --needed \
    webkit2gtk-4.1 gtk3 glib2 \
    libappindicator-gtk3 librsvg \
    base-devel curl wget file openssl
```

## Running

`web/dist` **must exist** before any desktop-crate cargo command —
Tauri's build script validates the `bundle.resources` path from
`tauri.conf.json` at compile time and hard-fails if it's missing.

```bash
cd web && npm ci && npm run build && cd ..
cd apps/flighttrace-desktop/src-tauri
cargo tauri dev              # native window with HMR-less UI
cargo tauri build            # installers under target/release/bundle/
cargo check                  # compile-only, still needs web/dist
```

## Web-asset resolution

At startup the shell resolves the SPA path in this order:

1. `FLIGHTTRACE_WEB_DIR` env var.
2. `AppHandle::path().resource_dir()/web-dist` — where
   `cargo tauri build` drops the SPA via `bundle.resources`.
3. `AppHandle::path().resource_dir()/_up_/web-dist` — some macOS
   bundle layouts.
4. `AppHandle::path().resource_dir()/web/dist` — fallback bundler
   layout.
5. `<current-exe dir>/web-dist` — portable release: drop `web-dist/`
   next to the binary.
6. `CARGO_MANIFEST_DIR/../../../web/dist` — repo-relative, works for
   `cargo tauri dev` from a checkout.

If none work, the shell exits with a numbered list of every path it
tried plus the recommended fix (`npm run build`, set
`FLIGHTTRACE_WEB_DIR`, or drop `web-dist/` next to the binary). No
blank window.

## Database location

Priority chain:

1. `FLIGHTTRACE_DATABASE` env var.
2. `AppHandle::path().app_data_dir()/flighttrace-desktop.db` — proper
   per-OS location:
   - **Linux**: `~/.local/share/com.flighttrace.desktop/flighttrace-desktop.db`
   - **macOS**: `~/Library/Application Support/com.flighttrace.desktop/flighttrace-desktop.db`
   - **Windows**: `%APPDATA%\com.flighttrace.desktop\flighttrace-desktop.db`
3. `./data/flighttrace-desktop.db` — fallback if `app_data_dir()`
   fails (rare — no `HOME`, no `%APPDATA%`).

Parent directories are created automatically.

## WSL2

`cargo tauri dev` renders through WSLg on Windows 11 (default install,
nothing extra needed). On Windows 10 you need a running X server
(VcXsrv, X410) with `DISPLAY` exported before launching the app.

WSL2 doesn't expose `/dev/dri`, so webkit2gtk's GPU-accelerated paths
fail their probe and log messages like:

```
libEGL warning: failed to get driver name for fd -1
MESA-LOADER: failed to retrieve device information
MESA: error: ZINK: failed to choose pdev
```

These are **cosmetic** — WebKit falls back to software rendering and
the window works fine.

As of issue 12 the shell auto-sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`
and `WEBKIT_DISABLE_COMPOSITING_MODE=1` for you when it detects WSL2
(via `WSL_DISTRO_NAME` / `WSL_INTEROP` env vars or `microsoft` in
`/proc/version`). Native Linux hosts are unaffected — the detection
only fires on WSL2, and existing values are respected so you can
override either variable by exporting a different value first.

To force the same defaults on a machine that isn't WSL2:

```bash
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
```

Both are also present as commented-out lines in `.env.example`.

## CI

The `desktop-check` job in `.github/workflows/ci.yml` runs
`cargo fmt --check`, `cargo check --locked`, and `cargo clippy
--all-targets -- -D warnings` inside the desktop crate on
`ubuntu-latest` with the apt bundle installed. It also builds
`web/dist` first (because of the `bundle.resources` compile-time
check). The job is **independent** of the `rust` and `web` jobs — a
red desktop check does not fail the main workspace or web gates.

`cargo tauri build` is not part of CI. Producing installer artefacts
on CI is out of scope for v0.1; the manual command above works on
any dev box with the system packages installed.
