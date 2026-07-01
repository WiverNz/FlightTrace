# Run modes

FlightTrace supports four run modes. The [top-level
README](../README.md) uses the **packaged web** mode as its primary
quickstart because it's the fastest way to see a rendered
`TAKEOFF_TIMEOUT` failure report. Use this document to pick the mode
that fits your task.

| Mode | Command shape | Best for |
| --- | --- | --- |
| **Packaged web** | `flighttrace serve --web-dir ./web/dist --open` | Demoing the product to someone; screenshots; one-process evaluation. |
| **Desktop (Tauri v2)** | `cd apps/flighttrace-desktop/src-tauri && cargo tauri dev` | A native window; same UI, no separate browser. |
| **Dev (two processes)** | `cargo run … serve` + `npm run dev` | Iterating on the frontend with HMR. |
| **CLI, API-only** | `flighttrace serve` | Backend dev; `curl`-driven exploration. |

All four modes speak the same [HTTP API](api.md) and store data in the
same SQLite schema. The frontend uses same-origin fetches when
`VITE_API_BASE_URL` is unset (packaged + desktop), falling back to the
value in `web/.env.development` (dev mode).

## Packaged web (primary)

One process serves both the API and the prebuilt SPA.

```bash
# One-time frontend build.
cd web && npm ci && npm run build && cd ..

# Start the packaged server.
cargo run -p flighttrace-cli -- serve \
    --database ./data/flighttrace.db \
    --web-dir ./web/dist \
    --open
```

Then open <http://127.0.0.1:8080>. `--open` launches the default
browser once the listener binds; drop it in headless environments.

- Client-side routes like `/runs/<id>` fall back to `index.html`, so
  React Router still owns them.
- Unknown `/api/*` paths return the standard JSON 404 envelope — the
  SPA fallback never swallows a bad API URL.
- Omit `--web-dir` and the behaviour degrades gracefully to API-only.

## Desktop (Tauri v2)

A native window that spawns the same Axum server on a random loopback
port and points a WebView at it. See [`docs/desktop.md`](desktop.md)
for prerequisites (Linux packages, WSL2 quirks, macOS/Windows notes).

```bash
cd web && npm ci && npm run build && cd ..
cargo install tauri-cli --version '^2'
cd apps/flighttrace-desktop/src-tauri && cargo tauri dev
```

The desktop crate resolves web assets in this priority order:

1. `FLIGHTTRACE_WEB_DIR` env var.
2. `<Tauri resource-dir>/web-dist` (bundled via `bundle.resources` in
   `tauri.conf.json`).
3. `<Tauri resource-dir>/_up_/web-dist` or `.../web/dist` (alternative
   bundler layouts).
4. `<exe dir>/web-dist` (portable — drop `web-dist/` next to the
   binary).
5. `<repo>/web/dist` via `CARGO_MANIFEST_DIR` (repo-relative fallback).

If none work the shell exits with a numbered list of every path it
tried plus the recommended fix. No blank window.

SQLite lives at the platform's app-data dir by default:
- Linux: `~/.local/share/com.flighttrace.desktop/flighttrace-desktop.db`
- macOS: `~/Library/Application Support/com.flighttrace.desktop/…`
- Windows: `%APPDATA%\com.flighttrace.desktop\…`

Override with `FLIGHTTRACE_DATABASE`.

## Dev (two processes)

Vite dev server on `:5173`, backend on `:8080`. Cross-origin fetches
against the CORS-allowed dev origin.

```bash
# Terminal 1 — API.
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db

# Terminal 2 — Vite dev server with HMR.
cd web && npm ci && npm run dev
```

Open <http://localhost:5173>. Best when you're iterating on the
frontend — HMR is live, and Vite handles source maps.

## CLI, API-only

Just the backend. Useful for `curl` / integration-test loops.

```bash
cargo run -p flighttrace-cli -- serve \
    --bind 127.0.0.1:8080 \
    --database ./data/flighttrace.db
```

Then hit the endpoints documented in [`api.md`](api.md). The
`/health` and `/api/*` paths behave the same as in the other three
modes.

## Environment variables

Every `serve` flag also reads a matching `FLIGHTTRACE_*` env var with
CLI-flag > env-var > default precedence. See
[`configuration.md`](configuration.md) for the full table and the
`.env` pattern.
