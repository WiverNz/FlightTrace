# Screenshots

This directory holds the images the top-level [README](../../README.md)
references. It currently contains **placeholders** — schematic layouts
in the FlightTrace palette with a diagonal `PLACEHOLDER` texture — so
the README never renders a broken image. Replace each file with a real
capture using the recipe below.

## Files

| File | Where it appears | What to capture |
| --- | --- | --- |
| `runs-list.png` | README §Screenshots | Runs list with the failed demo run at the top; `FAILED — TAKEOFF_TIMEOUT` badge visible. |
| `run-detail-failure-summary.png` | README §Screenshots | Run detail page, scrolled to the top so the Failure Summary card + AltitudeChart are fully visible. |
| `run-detail-telemetry.png` | README §Screenshots | Same run, scrolled so the full Telemetry Overview (altitude chart + vertical-speed panel + sample table) fills the frame. |
| `desktop-window.png` | README §Screenshots | `cargo tauri dev` window on the runs list, native title bar included. |

## Capture recipe

1. Build the frontend and start a fresh backend:

   ```bash
   cd web && npm ci && npm run build && cd ..
   rm -f ./data/flighttrace.db*
   cargo run -p flighttrace-cli -- serve \
       --bind 127.0.0.1:8080 \
       --database ./data/flighttrace.db \
       --web-dir ./web/dist \
       --open
   ```

2. In the browser (or the Tauri window for the desktop shot), click
   **Create demo failed run** at least once.

3. Resize the window to **1440 × 900**. This matches the
   `docs/release-checklist.md` §Screenshot checklist and keeps the
   layout consistent between captures.

4. Confirm the dark theme is active (it ships by default; don't
   override it).

5. Capture each shot per the table above. On Linux `gnome-screenshot -a`
   or `flameshot gui` work; on macOS use `Cmd + Shift + 4`; on Windows
   use `Win + Shift + S`.

6. Save each file as `.png` under this directory, keeping the exact
   filenames.

7. Compress each file to under ~200 KB (target ~100 KB). `pngcrush`,
   `oxipng`, or `pngquant --quality 60-80` all work well and preserve
   the palette. If a file goes over 200 KB, the README-render latency
   on GitHub becomes noticeable.

8. Never commit vehicle telemetry from a real flight, personal file
   paths, or window titles containing anything private. The demo run
   is purely synthetic — capture the demo, not a live session.

## Regenerating the placeholders

```bash
# From the repo root. Overwrites the four .png files with fresh
# placeholders in the current palette. Handy after a palette change.
python3 scripts/generate_screenshot_placeholders.py
```

The script mirrors the palette from `web/src/styles/tokens.css` and
`docs/tech-stack.md` — update it in lockstep when either of those
change.
