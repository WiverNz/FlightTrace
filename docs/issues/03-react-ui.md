# Issue 3 — React UI

## Summary

Build the first FlightTrace web UI: a React + TypeScript + Vite app that talks to the Axum API from issue 2. The UI lets a developer create a demo failed run, see it in a runs list, open it, and understand why it failed without reading source code. After this issue the full v0.1 demo loop works end-to-end on demo data, with no MAVLink, no SITL, and no real vehicle.

## Product goal

After this issue, an engineer can:

1. Start the backend (`cargo run -p flighttrace-cli -- serve`).
2. Start the frontend (`npm run dev` in `web/`).
3. Open `http://localhost:5173` in a browser.
4. Click **"Create demo failed run"**.
5. See a new failed run appear at the top of the runs list, badged `FAILED — TAKEOFF_TIMEOUT`.
6. Click the run and see the run detail page with:
   - mission status and failure reason banner
   - failure report card (summary, evidence list, likely cause)
   - mission timeline (phase transitions)
   - assertion table (name, outcome, detail)
   - telemetry overview (altitude over time as a simple chart or table)
   - raw normalized event log (timestamp + kind + payload)
7. Refresh the page and the run is still there.

The UI must feel like a **developer debugger**: dense, monospace-leaning, evidence-first. Not a marketing dashboard.

## Technical scope

- Create the `web/` React + TypeScript + Vite project.
- Implement an API client typed against the issue-2 DTOs.
- Implement two routes: runs list (`/`) and run detail (`/runs/:id`).
- Implement the components listed under "Screens" in the PRD.
- Style with a small, consistent CSS (CSS modules or plain CSS). No design system, no Tailwind required (allowed if trivial).
- Keep the dependency footprint small.

No MAVLink, no auth, no live streaming in this issue. Polling for refresh is acceptable.

## Files and modules to create or modify

```
web/
  package.json
  tsconfig.json
  vite.config.ts
  index.html
  src/
    main.tsx
    App.tsx
    router.tsx
    api/
      client.ts                                 # fetch wrapper, base URL from env
      runs.ts                                   # listRuns, getRun, getEvents, getAssertions, getFailureReport, createDemoRun
    types/
      api.ts                                    # TS mirror of the DTOs
    pages/
      RunsListPage.tsx
      RunDetailPage.tsx
    components/
      RunsTable.tsx
      CreateDemoRunButton.tsx
      StatusBadge.tsx
      FailureReportCard.tsx
      MissionTimeline.tsx
      AssertionTable.tsx
      TelemetryOverview.tsx
      RawEventLog.tsx
      EvidenceList.tsx
      LoadingState.tsx
      ErrorState.tsx
    styles/
      globals.css
      tokens.css                                # colors, spacing, monospace font
  .env.development                              # VITE_API_BASE_URL=http://localhost:8080
```

## Detailed implementation steps

1. **Scaffold.**
   - `npm create vite@latest web -- --template react-ts` (or equivalent), then prune the starter content.
   - Add `react-router-dom` for routing. No state library, no data-fetch library. Use `fetch` + React hooks.
   - Set `VITE_API_BASE_URL` in `.env.development` to `http://localhost:8080`.
   - Add scripts: `dev`, `build`, `lint`, `preview`.

2. **API client and types.**
   - `types/api.ts` mirrors every DTO from issue 2 in TypeScript (`MissionRunDto`, `NormalizedEventDto`, `TelemetrySampleDto`, `AssertionResultDto`, `FailureReportDto`, `EvidenceDto` union, etc.). Use the same `camelCase` field names.
   - `api/client.ts` exposes a small `request<T>(path, init?): Promise<T>` that prepends `VITE_API_BASE_URL`, sets `Accept: application/json`, throws a typed `ApiError` (with `status` and the server's `error.code`/`error.message`).
   - `api/runs.ts` wraps each documented endpoint:
     - `listRuns(): Promise<MissionRunDto[]>`
     - `getRun(id): Promise<MissionRunDto>`
     - `getEvents(id): Promise<NormalizedEventDto[]>`
     - `getAssertions(id): Promise<AssertionResultDto[]>`
     - `getFailureReport(id): Promise<FailureReportDto>`
     - `createDemoRun(): Promise<{ id: string }>`

3. **Routing.**
   - `/` → `RunsListPage`
   - `/runs/:id` → `RunDetailPage`
   - 404 fallback → a simple "Run not found" state with a link back to `/`.

4. **Runs list page.**
   - Header: product name "FlightTrace", subtitle "mission debugger".
   - Top-right action: `<CreateDemoRunButton />`. On click: call `createDemoRun()`, then refresh the list, then navigate to the new run's detail page.
   - `<RunsTable />` columns: `Started`, `Mission`, `Vehicle`, `Source`, `Status`, `Failure reason`, `Duration`. Status rendered via `<StatusBadge />` (`running` neutral, `succeeded` green, `failed` red). Whole rows are clickable links to `/runs/:id`.
   - Empty state: short text and a hint to click the demo button.
   - Loading and error states use shared `<LoadingState />` and `<ErrorState />` components.

5. **Run detail page.**
   - Header strip: run id (short), vehicle, mission, source, started/ended timestamps, prominent status badge with failure reason.
   - `<FailureReportCard />`: shown only if the run is `failed`. Title `Mission failed: <REASON>`. Renders the summary, the `<EvidenceList />`, and the likely-cause paragraph in a distinct callout.
   - `<MissionTimeline />`: vertical or horizontal sequence of phase transitions with timestamps. Highlight the transition into `Failed`.
   - `<AssertionTable />`: columns `Name`, `Outcome`, `Detail`, `Evaluated at`. Sort failing first. Outcome badges: `pass` green, `warn` yellow, `fail` red.
   - `<TelemetryOverview />`: minimal but useful. Acceptable v0.1 implementation is a plain table of `time | altitude_m | vertical_speed_mps | mode | armed | battery%` plus a tiny inline SVG sparkline of altitude. A chart library is allowed if it stays small (e.g. `recharts`); not required.
   - `<RawEventLog />`: monospace, virtualized only if it noticeably lags (the demo run is small). Each row: ISO timestamp, kind, payload (JSON-stringified). Filter input that narrows by `kind` substring.
   - All sections fetch in parallel on mount; render placeholders per section as data arrives.

6. **Styling.**
   - Single dark theme. Neutral palette (slate/zinc) with semantic colors for status (red/yellow/green).
   - Monospace font (`JetBrains Mono`, `IBM Plex Mono`, or system `ui-monospace`) for timestamps, IDs, raw payloads.
   - Sans-serif for body. Dense spacing. No animations beyond hover.
   - Make sure the UI does not look like a stock template. It should look like a debugger.

7. **API base URL handling.**
   - All requests go through `client.ts`. Reading `import.meta.env.VITE_API_BASE_URL` happens in exactly one place.
   - During dev, Vite proxies are optional; using the env var is simpler and matches the CORS allowlist from issue 2.

8. **Build and lint.**
   - `npm run build` produces a `web/dist` bundle.
   - `npm run lint` runs ESLint (default Vite TS config is acceptable; add `react-hooks` and `react-refresh` rules if missing).
   - Add `web/node_modules` and `web/dist` to `.gitignore`.

## Acceptance criteria

- [ ] `web/` is a working Vite + React + TypeScript app.
- [ ] `npm run dev` starts the dev server on port 5173 and renders the runs list.
- [ ] Clicking **Create demo failed run** calls `POST /api/demo/run`, then navigates to the new run's detail page.
- [ ] Runs list shows the new run with status `FAILED` and reason `TAKEOFF_TIMEOUT`.
- [ ] Run detail page shows the failure report card, mission timeline, assertion table, telemetry overview, and raw event log — all populated from the API.
- [ ] The failure report card displays the summary line, evidence list, and likely cause paragraph from issue 1's demo.
- [ ] At least one assertion row is rendered with outcome `fail`.
- [ ] Refreshing the page preserves all rendered data (no client-only state required for correctness).
- [ ] `npm run build` succeeds with no TypeScript errors.
- [ ] `npm run lint` succeeds.
- [ ] The UI works without any backend mock — only the real Axum server from issue 2.

## Tests required

- Manual UI verification of the full demo loop in a browser is required and must be reported in the PR description (golden path: open UI, create demo, see failure report, see assertions, see timeline).
- TypeScript build is the primary static check; do not invest in heavy component-test infrastructure for v0.1.
- Lightweight unit tests are welcome but not required. If added, prefer `vitest` and test pure helpers in `api/` and small components like `StatusBadge`.

## Out of scope

- No live streaming (SSE/WebSocket) — polling on demand is fine.
- No map / 3D replay.
- No heavy charting libraries. A sparkline or small chart is enough.
- No auth, no user settings, no themes other than the default.
- No i18n.
- No mobile-first responsive design. A reasonable desktop layout is the v0.1 target.
- No deployment configuration. `npm run dev` is the supported entrypoint.

## Notes for future issues

- **Issue 4** (MAVLink) will produce runs whose data grows over time. The run detail page should later support live updates; design the data-fetching layer so adding SSE/WebSocket later is a localized change. A simple "refresh" button on the run detail page is acceptable for v0.1.
- **Issue 5** (assertions) will add richer assertion metadata (which evidence each assertion cites). The `AssertionTable` should be ready to grow an expandable row.
- **Issue 6** (docs) will add screenshots; keep the UI presentable so screenshots are not embarrassing.
- Do not introduce a state-management library yet. If it becomes necessary in issue 4 or 5, propose it in that issue's PR.
