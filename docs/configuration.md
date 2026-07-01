# Configuration

Every `serve` option can also come from the environment. Precedence is
**CLI flag > environment variable > built-in default**. Same variables
apply to the CLI and the desktop shell.

## Environment variables

| Variable | CLI flag | Default | Notes |
| --- | --- | --- | --- |
| `FLIGHTTRACE_BIND` | `--bind` | `127.0.0.1:8080` | CLI only. The desktop shell always binds `127.0.0.1:0`. |
| `FLIGHTTRACE_DATABASE` | `--database` | `./data/flighttrace.db` (CLI) / OS app-data dir (desktop) | Parent dir is created if missing. See [`desktop.md`](desktop.md) for the OS-specific paths. |
| `FLIGHTTRACE_MAVLINK` | `--mavlink` | unset | UDP endpoint, e.g. `udp://127.0.0.1:14550`. Absent → demo-only. |
| `FLIGHTTRACE_MAVLINK_DIALECT` | `--mavlink-dialect` | `common` | `common`, `ardupilotmega`, or `px4`. |
| `FLIGHTTRACE_MISSION` | `--mission` | unset | Path to a mission YAML for MAVLink-session evaluation. |
| `FLIGHTTRACE_WEB_DIR` | `--web-dir` | unset (API-only, CLI) / desktop resolver (desktop) | Directory containing `index.html`. |
| `FLIGHTTRACE_OPEN` | `--open` | unset | CLI only. Opens the default browser after bind. |
| `RUST_LOG` | — | `info` | Standard `tracing_subscriber` env filter. |
| `RUST_BACKTRACE` | — | unset | Normal Rust meaning. |

## The `.env` pattern

`clap` reads the process environment; it does **not** load `.env`
files on its own. Copy the example and export it into the current
shell before running the CLI:

```bash
cp .env.example .env
set -a
source .env
set +a
cargo run -p flighttrace-cli -- serve
```

For example, `--bind 127.0.0.1:8089` overrides
`FLIGHTTRACE_BIND=127.0.0.1:8088`. The root `.env` is gitignored;
`.env.example` is safe to commit.

## VS Code

The VS Code launch configurations under `.vscode/launch.json` read the
same root `.env` file via `envFile`. Its example uses debug bind
address `127.0.0.1:8088`, leaving the README quickstart on port
`8080`. MAVLink and mission variables are commented out by default,
so the `CLI: serve demo-only` profile remains demo-only; the MAVLink
profile supplies them as explicit CLI flags.

## Frontend

`web/.env.development` sets `VITE_API_BASE_URL` for the Vite dev
server. When it's **empty or unset** the frontend uses same-origin
fetches, which is what packaged web mode and the desktop shell rely
on. Do not set `VITE_API_BASE_URL` at build time unless you have a
specific cross-origin deployment in mind.

## Desktop-only

The Tauri shell honours two extra env vars useful on WSL2 (see
[`desktop.md`](desktop.md) for context):

```bash
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
```

Both are commented out in `.env.example`; harmless on native
Linux/macOS/Windows.
