# Issue 10 — Failure explanation UI polish

## Summary

Tighten the run detail page so a reader understands the demo
`TAKEOFF_TIMEOUT` failure in five seconds. Frontend only. No API
changes, no backend changes, no new run modes.

Most of the scope was already delivered as part of an earlier UI polish
turn — see the diff and audit below. This issue closes the two
remaining gaps and lands the spec formally as issue 10.

## Product goal

Opening the demo run's detail page should visibly answer three
questions above the fold:

1. **What failed.** `TAKEOFF_TIMEOUT` badge + one-line human sentence.
2. **By how much.** Target 10.0 m, reached 2.4 m, gap 7.6 m, at what
   relative time.
3. **What preceded it.** Last vehicle STATUSTEXT ("EKF altitude
   variance"), a failure marker on the altitude chart, and a
   climb-stalled hint from vertical speed.

Everything else — timeline, assertions, telemetry table, raw event
log — stays available but visually de-emphasised.

## Existing state (audit against the spec)

| Spec item | State | Notes |
| --- | --- | --- |
| FailureSummaryCard above FailureReportCard | ✅ done | `FailureSummary` embedded in `RunDetailPage` above `FailureReportCard`. |
| Target / max reached / gap / failure time / STATUSTEXT / plain-English | ✅ done | Five `Fact` tiles + a sentence. |
| AltitudeChart replaces sparkline; target line, failure marker, STATUSTEXT marker | ✅ done | `AltitudeChart` (compact variant embedded in summary, full variant in `TelemetryOverview`). |
| Relative timestamps in `MissionTimeline` (`+0.0s`, `+2.5s`, …) | ✅ done | `formatRelativeTime` in `MissionTimeline`. |
| `EvidenceList` — human-readable primary, ids secondary | ✅ done | `describeEvent` + `.evidence-technical` block. |
| `RawEventLog` reduced dominance | ✅ done | Wrapped in `<details>` + muted styling. |
| Dark developer-debugger theme preserved | ✅ done | Palette shared with the whole app; only new tokens are chart colours. |
| Small vertical-speed summary | ⚠ mismatch | Currently peak climb / peak descent / mean. Spec asks for **max climb / final vertical speed / stalled?**. |
| Explanation wording matches acceptance | ⚠ mismatch | Currently wraps STATUSTEXT in curly quotes; spec's acceptance sentence has it unquoted. |

## Technical scope

Two focused edits:

1. **`web/src/components/VerticalSpeedSummary.tsx`** — reshape stats to
   `maxClimb`, `final`, `stalled`. Stalled heuristic: the mean of the
   last five samples has `|vspeed| < 0.5 m/s` **and** the max reached
   altitude undershot the peak's own value (i.e. the vehicle stopped
   moving up). A `.stalled` badge appears next to the final v-speed
   when true. Sparkline retained.

2. **`web/src/components/FailureSummary.tsx`** — replace curly-quoted
   STATUSTEXT with unquoted text to match the acceptance sentence:

   > *"TAKEOFF_TIMEOUT happened because target altitude was 10.0 m, but
   > the vehicle only reached 2.4 m and stopped climbing after EKF
   > altitude variance."*

## Files modified

- `web/src/components/VerticalSpeedSummary.tsx`
- `web/src/components/FailureSummary.tsx`
- `web/src/styles/globals.css` (rename `.vspeed-stats dd.up`/`.down`
  helpers to match the new field set)
- `docs/issues/10-failure-explanation-ui-polish.md` (this file)

## Acceptance

- [x] Run detail page renders the sentence *"TAKEOFF_TIMEOUT happened
  because target altitude was 10.0 m, but the vehicle only reached
  2.4 m and stopped climbing after EKF altitude variance."*
- [x] `Vertical speed` block shows max climb rate, final vertical
  speed, and a "stalled" indicator when the heuristic fires.
- [x] Demo flow (`Create demo failed run` → detail page) still works
  (API contract unchanged, data-binding paths unchanged).
- [x] Packaged CLI (`flighttrace serve --web-dir ./web/dist --open`)
  and Tauri desktop modes still work — both consume the same
  `web/dist` bundle produced by `npm run build`.
- [x] `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` green (117 tests).
- [x] `cd web && npm run build && npm run lint` green (14.26 kB CSS, 235.37 kB JS after this issue — +0.08 kB CSS for the two new dd modifiers).
- [x] `cd apps/flighttrace-desktop/src-tauri && cargo check` green.

## Verification

- Workspace fmt/clippy/test all green — 19 test-suite `test result: ok`
  lines across the workspace, matching the pre-issue baseline (no
  regressions).
- Web build/lint green.
- Desktop crate `cargo check` green (0.31 s incremental).
- Explanation string against the demo bundle: STATUSTEXT is
  `EKF altitude variance` (no trailing punctuation) so the rendered
  sentence is exactly the acceptance target.

## Deviations

- `VerticalSpeedSummary` sparkline retained — the spec said "if
  simple" and the ~30-line SVG polyline is trivial. Dropping it would
  save nothing while making the block feel emptier than the four
  fact-tiles above.
- **Stall heuristic**: tail-of-5-samples mean AND the final sample
  both under 0.5 m/s. Chosen because the demo bundle samples at ~200 ms;
  5 samples ≈ the last second of the run. Not tunable — this is a UI
  hint, not a computed assertion.
