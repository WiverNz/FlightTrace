# Issue 13 — GUI-by-default packaged binary

## Summary

Make the packaged CLI binary behave like an app when a user runs it
with no arguments. Today, double-clicking `flighttrace.exe` (or running
`./flighttrace` from an extracted release archive) prints CLI help and
exits — from the user's perspective, nothing happens.

After this issue, running the binary bare **starts the FlightTrace UI**
in the default browser when `web-dist/` is available. Explicit
subcommands (`demo`, `serve`, `--help`, `--version`) keep the exact
behaviour they had before.

No new run modes, no API changes, no Tauri replacement. This is CLI
ergonomics.

## Product goal

- `./flighttrace` from a release archive → app opens in browser.
- `flighttrace.exe` double-clicked → same.
- `flighttrace --help` / `--version` / `demo` / `serve …` → identical
  to today.
- Missing `web-dist/` → a single-sentence actionable error, not a
  crash or a silent no-op.
- Windows users get a `start-flighttrace.bat` in the archive so
  double-click keeps the terminal window open long enough to read the
  URL / any error.

## Technical scope

1. **`Cli.command` becomes `Option<Command>`.** With clap's derive
   API, `None` means "user typed no subcommand". Route `None` and a
   new explicit `Gui` variant to the same handler.

2. **New `Command::Gui`.** Documented in `--help` so power users can
   see the mode name. No arguments; everything comes from env/CWD/
   filesystem.

3. **New `run_gui()`** flow:
   - Resolve `web-dist` via a four-step priority chain (see below).
     Missing → exit with the actionable error.
   - Resolve database from `FLIGHTTRACE_DATABASE` or
     `./data/flighttrace.db`.
   - Delegate to `run_serve(bind = 127.0.0.1:0, database, web_dir,
     open_browser = true, announce_stdout = true)`.
   - The new `announce_stdout` flag prints
     `FlightTrace UI: http://127.0.0.1:<port>/` to **stdout** after the
     listener binds, so the user (and the release-smoke script) can
     see and use the URL immediately.

4. **`resolve_gui_web_dir()`** — pure function (with a public
   `_with(env_override, cwd, exe_dir)` twin for tests) that walks:
   1. `FLIGHTTRACE_WEB_DIR` env var.
   2. `./web-dist` in the current working directory.
   3. `<exe dir>/web-dist` — the release-archive shape.
   4. `./web/dist` — repo checkouts.

   On miss, returns
   `No web UI found. Place web-dist next to the binary, set
   FLIGHTTRACE_WEB_DIR, or run \`flighttrace serve\` for API-only mode.`
   plus a numbered list of every path it tried.

5. **`assets/start-flighttrace.bat`** — committed to the repo,
   copied into Windows release archives by `release.yml`:

   ```bat
   @echo off
   cd /d "%~dp0"
   flighttrace.exe
   pause
   ```

6. **Docs.**
   - README run-modes table gets a **"Downloaded release"** row at
     the top, before packaged / desktop / dev / API-only.
   - `docs/release-notes/v0.1.0.md` updated to mention the double-
     click behaviour and the batch file.

7. **Tests.**
   - Clap parse tests: no args, `gui`, `demo`, `serve`, `--help`
     produce the expected shapes.
   - `resolve_gui_web_dir_with(...)` unit tests for each candidate
     hit and for the miss path.
   - Env-var precedence tests continue to hold (their existing
     `parse_serve` helper is untouched by the `Option<Command>`
     change because it explicitly `try_parse_from(["flighttrace",
     "serve", …])`).

8. **Release-smoke extension.** `scripts/release-smoke.sh` gains a
   check that runs the binary bare in a scratch directory holding
   `flighttrace` + `web-dist`, parses the printed URL from stdout,
   and hits `/health`, `POST /api/demo/run`, and the SPA-fallback
   endpoint. Old checks (explicit `serve --web-dir`) stay intact.

## Files changed

- `crates/flighttrace-cli/src/main.rs` — CLI refactor + `run_gui` +
  `resolve_gui_web_dir` + `announce_stdout`.
- `assets/start-flighttrace.bat` — new.
- `.github/workflows/release.yml` — Windows package step copies the
  batch file.
- `README.md` — new run-mode row + short "Downloaded release"
  paragraph.
- `docs/release-notes/v0.1.0.md` — mention double-click behaviour.
- `scripts/release-smoke.sh` — new `check_gui_mode` block.
- `docs/issues/13-gui-default-packaged-binary.md` — this file.

## Acceptance

- [x] `flighttrace` with no args, from a directory that contains
  `web-dist/index.html`, prints `FlightTrace UI: http://127.0.0.1:<port>/`
  to stdout and opens the default browser at that URL — verified by
  `release-smoke.sh` GUI block.
- [x] Same command from a directory with **no** web-dist prints the
  actionable error and exits non-zero — verified by the smoke's
  `"No web UI found"` grep on the failure path.
- [x] `flighttrace gui` behaves identically to no-arg — both routed
  through the same `None | Some(Command::Gui) => run_gui()` arm.
- [x] `flighttrace --help`, `flighttrace --version`, `flighttrace
  demo`, and every `flighttrace serve …` form still behave exactly
  as before — verified by the `help_and_version_short_circuit…`,
  `existing_demo_and_serve_subcommands_still_parse`, and existing
  6-test env-precedence suite.
- [x] Windows release archive contains `start-flighttrace.bat`
  alongside `flighttrace.exe` — copied by the Windows package step
  in `.github/workflows/release.yml`.
- [x] `scripts/release-smoke.sh` runs both the explicit-serve checks
  and the new no-arg check and exits 0 — 9 OK lines, 0 FAIL.
- [x] All quality gates stay green.

## Verification

- **CLI tests**: `cargo test -p flighttrace-cli` — **15 passed / 0
  failed**, up from 6 (+9 for issue 13: two clap-parse tests, one
  help-short-circuit test, four `resolve_gui_web_dir_with` priority
  tests, one `is_valid_web_dir` check, one demo/serve regression
  guard).
- **Workspace**: `cargo fmt --check` ✅, `cargo clippy --workspace
  --all-targets -- -D warnings` ✅, `cargo test --workspace` ✅ (19
  `test result: ok` lines).
- **Web**: `npm run build` ✅ (17.28 kB CSS, 237.85 kB JS — no
  regression), `npm run lint` ✅.
- **Desktop**: `cargo check` ✅, `cargo clippy --all-targets -- -D
  warnings` ✅.
- **Release smoke**: 9/9 OK. Old checks (6) + new GUI-mode
  successful-run check + `/` HTML check + missing-web-dist error
  check.

## Deviations

- **`run_serve` gained an 8th argument** (`announce_stdout: bool`),
  tripping `clippy::too_many_arguments`. Suppressed with an inline
  `#[allow(...)]` and a comment explaining the tradeoff. Bundling
  the args into a struct would just shift definition cost to every
  call site — the function is private with three callers.
- **`open_url_in_browser` is best-effort** in GUI mode too, same as
  when `--open` is passed to `serve`. If `xdg-open` / `open` /
  `cmd /c start` fails (headless environment), the server keeps
  running and the printed URL is enough. This matches issue 07's
  contract.
- **`FLIGHTTRACE_OPEN` is *not* consulted by GUI mode.** GUI mode
  always tries to open the browser (that's the point). Setting
  `FLIGHTTRACE_OPEN=0` won't disable it — the flag is `serve`-only.
