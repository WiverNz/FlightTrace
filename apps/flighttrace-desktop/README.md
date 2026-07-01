# flighttrace-desktop

Tauri v2 desktop shell for FlightTrace. See
[`docs/issues/08-tauri-desktop-shell.md`](../../docs/issues/08-tauri-desktop-shell.md)
for the full spec, and the top-level [`README.md`](../../README.md#run-modes)
for run-mode context.

Not a workspace member — Tauri's Linux link-time deps (`webkit2gtk`, GTK,
etc.) are opt-in. Build from inside this directory:

```bash
cd apps/flighttrace-desktop/src-tauri
cargo build            # requires system deps installed
cargo tauri dev        # requires `cargo install tauri-cli --version ^2`
```
