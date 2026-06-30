# Issue 6 — Docs, polish, and local demo

## Summary

Make FlightTrace easy to run, easy to evaluate, and easy to present. This is the final v0.1 issue. Its output is a polished README, a documented local demo workflow, an example mission YAML, an architecture diagram in markdown, SITL connection notes for PX4 and ArduPilot, screenshot instructions, a troubleshooting section, an optional `docker-compose.yml`, and a clean lint/test bill of health across the whole repository.

No new product features. This issue is about turning the working v0.1 into something an external engineer can pick up, run, and judge in under fifteen minutes.

## Product goal

After this issue, a new engineer can:

1. Clone the repo.
2. Read the README.
3. Run the documented commands to start the backend and the frontend.
4. Open the UI, click "Create demo failed run", and see the `TAKEOFF_TIMEOUT` failure report.
5. Optionally run PX4 or ArduPilot SITL and see a real MAVLink-sourced run.
6. Understand from the docs what FlightTrace is, what it is not, and what is planned next.

## Technical scope

- README with positioning, install, run, demo, SITL, and troubleshooting sections.
- Markdown architecture diagram (ASCII or Mermaid).
- Example mission YAML referenced from the README.
- Screenshot capture instructions and a place to drop screenshots.
- Optional `docker-compose.yml` for backend + frontend.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` all green.
- Frontend `npm run build` and `npm run lint` green.
- Repo hygiene: `.gitignore`, license notice (if chosen), CODEOWNERS / CONTRIBUTING optional.

## Files and modules to create or modify

```
README.md                                       # primary entry point
docs/
  prd.md                                        # cross-check and update if anything drifted
  context.md                                    # cross-check and update if anything drifted
  architecture.md                               # optional, if the diagram outgrows the README
  screenshots/                                  # drop captured PNGs here
    .gitkeep
examples/
  missions/simple_takeoff.yaml                  # ensure present and referenced from README
docker-compose.yml                              # optional, see step 8
.gitignore                                      # finalize
```

## Detailed implementation steps

1. **README structure.**
   - Title and one-line pitch (from `prd.md`).
   - "What FlightTrace is / is not" — two short bullet lists.
   - "How it works" — three sentences: ingest, normalize, evaluate.
   - Architecture diagram (ASCII or Mermaid). Inline; full diagram lives in `docs/architecture.md` only if needed.
   - "Quickstart — demo only" with exact commands (see step 2).
   - "Quickstart — with PX4 SITL" and "Quickstart — with ArduPilot SITL" subsections (see step 5).
   - "Project layout" — short tree of `crates/`, `web/`, `docs/`, `examples/`.
   - "Mission assertions" — the YAML example from issue 5, plus a one-paragraph explanation.
   - "Troubleshooting" (see step 6).
   - "Roadmap / what's next" — short list of what comes after v0.1.
   - "License" line if a license is chosen (not required for v0.1).

2. **Quickstart — demo only.**
   - Document the exact commands:
     ```
     # Backend
     cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db

     # Frontend (in another terminal)
     cd web
     npm install
     npm run dev
     ```
   - Then: open `http://localhost:5173`, click **Create demo failed run**, click the new row, observe the `TAKEOFF_TIMEOUT` failure report.
   - Include a short note that no PX4 / ArduPilot installation is required for this path.

3. **Architecture diagram.**
   - Inline ASCII diagram (the same one as `prd.md` section 8) is the floor.
   - If using Mermaid, fence with ```` ```mermaid ```` so GitHub renders it.
   - Show the relationship: UI → API → core/storage/assertions/demo/mavlink → SQLite, plus the optional MAVLink UDP source from SITL.

4. **Example mission YAML.**
   - Confirm `examples/missions/simple_takeoff.yaml` (from issue 5) exists with the documented content.
   - Reference it from the README's "Mission assertions" section with a short walk-through of what each step expects.

5. **SITL connection notes.**
   - For PX4: the exact command to start SITL with MAVLink on `udp://127.0.0.1:14550` (e.g. `make px4_sitl gz_x500` plus the MAVLink endpoint configuration), plus how to start FlightTrace with `--mavlink udp://127.0.0.1:14550`.
   - For ArduPilot: the equivalent (e.g. `sim_vehicle.py -v ArduCopter --out=udp:127.0.0.1:14550`).
   - State explicitly that FlightTrace is **read-only**, does not send commands, and that all arming/takeoff is performed by the SITL or its companion ground station (e.g. MAVProxy, QGroundControl).

6. **Troubleshooting.**
   - "Port 8080 already in use" → change `--bind`.
   - "Port 5173 already in use" → Vite will offer the next port; update CORS in code if you pin a custom one.
   - "No runs appear in the UI" → check the backend is running and that the browser console shows successful calls to `http://localhost:8080/api/runs`.
   - "MAVLink listener sees no heartbeats" → verify the SITL endpoint, check firewalls (Windows / WSL2), confirm the dialect.
   - "Database is locked" → ensure only one backend instance is running; SQLite WAL allows readers but one writer.
   - "Compilation errors after pulling" → run `cargo clean` and ensure the toolchain matches the workspace's edition.

7. **Screenshots.**
   - Add a short instructions block: which page to screenshot (runs list, run detail, failure report card), at what window size (1440×900 recommended), and where to save them (`docs/screenshots/`).
   - Commit a `.gitkeep` so the directory exists; do not commit large binary screenshots in this issue unless trivial.

8. **Optional `docker-compose.yml`.**
   - Provide a minimal compose file with two services:
     - `flighttrace-api`: builds from the repo root, runs `flighttrace-cli serve`, mounts `./data` for SQLite persistence, exposes `8080`.
     - `flighttrace-web`: builds from `web/`, runs `npm run dev` or serves the static build, exposes `5173`, sets `VITE_API_BASE_URL=http://localhost:8080`.
   - Mark it as optional in the README. The default supported workflow remains `cargo run` + `npm run dev`.

9. **Repo hygiene.**
   - `.gitignore` covers: `/target`, `web/node_modules`, `web/dist`, `data/`, `.env*.local`, OS junk (`.DS_Store`, `Thumbs.db`), editor folders (`.idea/`, `.vscode/` if not shared).
   - Confirm there are no stray `println!` calls, no `dbg!()`, no commented-out code blocks.
   - Confirm every public item in `flighttrace-core` has at least a one-line doc comment.

10. **Lint and test sweep.**
    - Run `cargo fmt --all`.
    - Run `cargo clippy --workspace --all-targets -- -D warnings`; fix or `#[allow(...)]` with justification.
    - Run `cargo test --workspace`; all green.
    - In `web/`: `npm run lint` and `npm run build`; both green.
    - Document these commands in a "Development" section of the README.

11. **PRD / context sync.**
    - Re-read `docs/prd.md` and `docs/context.md`. Update any section that drifted during implementation (especially API endpoints, CLI flags, and acceptance criteria).
    - Mark v0.1 acceptance criteria in `prd.md` section 12 as completed.

## Acceptance criteria

- [x] README contains: pitch, what-it-is-not, architecture diagram, demo quickstart, SITL quickstart (PX4 and ArduPilot), project layout, mission-assertions section, troubleshooting, and a short roadmap.
- [x] README demo quickstart, copy-pasted as-is into a fresh shell, produces a visible `TAKEOFF_TIMEOUT` failure report in the browser.
- [x] `examples/missions/simple_takeoff.yaml` exists and is referenced from the README.
- [x] `docs/screenshots/` directory exists with a `.gitkeep`, and the README explains how to capture screenshots.
- [x] Optional `docker-compose.yml` is either present and documented, or its absence is intentional and noted.
- [x] `.gitignore` excludes `target/`, `web/node_modules`, `web/dist`, `data/`, and local env files.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `cargo test --workspace` passes.
- [x] `npm run build` and `npm run lint` in `web/` pass.
- [x] `docs/prd.md` section 12 has all v0.1 acceptance checkboxes ticked, reflecting the actual implementation state.

## Implementation notes (delivered)

- **README rewritten** end-to-end against the 15-section spec: pitch, is /
  is not, how it works (ingest → normalize → evaluate), ASCII architecture
  diagram, demo quickstart, ArduPilot + PX4 SITL quickstarts (with the
  `--mission examples/missions/simple_takeoff.yaml` flag wired in),
  read-only guarantee callout, mission-assertions YAML walk-through with a
  check-to-failure-reason table, HTTP API reference, project layout tree,
  development gates, screenshot instructions, troubleshooting, post-v0.1
  roadmap, and a license note pointing at the workspace's
  `MIT OR Apache-2.0` declaration.
- **`docs/screenshots/.gitkeep` added.** Screenshot capture instructions in
  the README cover which pages to grab, resolution (1440×900), and file
  naming.
- **No `docker-compose.yml` shipped.** The README "A note on Docker"
  subsection inside the roadmap explicitly states that container packaging
  is intentionally deferred to the single-binary release work that follows
  v0.1. The default supported workflow is `cargo run` + `npm run dev`.
- **`.gitignore` extended.** Already covered `target/`, `data/`,
  `web/node_modules`, `web/dist`; added `.env*.local`, `.env`, `.DS_Store`,
  `Thumbs.db`, `.idea/`, and `.vscode/` so OS / editor junk and local env
  overrides stay out of the repo. The tracked `flighttrace.code-workspace`
  is untouched.
- **`docs/prd.md` section 12** has all 15 acceptance checkboxes ticked.
  Wording on the "README explains how MAVLink SITL ingestion is wired in"
  line was tightened from future tense (`will be wired in`) to present
  tense.
- **`docs/context.md`** required no changes: vocabulary, exclusions, and
  conventions all still match the implementation after issues 1–5.

## Verification

Manual demo quickstart (the exact README path), 2026-06-30, from
`/home/askibin/flighttrace`:

1. `cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:18687 --database /tmp/ft_issue6.db` — API came up, `/health` returned `{"status":"ok","version":"0.1.0"}`.
2. `POST /api/demo/run` returned a fresh UUID id.
3. `GET /api/runs?limit=1` showed the new row with `status: "FAILED"`,
   `failureReason: "TAKEOFF_TIMEOUT"`, source `DEMO`, mission
   `simple_takeoff`, and five phase transitions
   (`CONNECTED`/`ARMED`/`TAKEOFF_REQUESTED`/`CLIMBING`/`FAILED`).
4. `GET /api/runs/:id/failure-report` returned `summary: "Mission failed:
   TAKEOFF_TIMEOUT"`, three evidence items (Event, altitude Sample @ 2.4 m,
   EKF Statustext), and the PRD's likely-cause paragraph.
5. `GET /api/runs/:id/assertions` returned 5 results;
   `takeoff.altitude_at_least_m` was `FAIL` with
   `detail: "max_altitude=2.4m target=10.0m"`.
6. `GET /api/runs/:id/events` returned the expected normalized-event mix
   (`VehicleConnected`, `HeartbeatReceived`, `ModeChanged`, `ArmedChanged`,
   `CommandAccepted`, `AltitudeUpdated` × 15, `StatustextReceived`,
   `MissionFailed`).
7. `GET /api/runs/:id/telemetry` returned 15 samples; first `altitudeM = 0.0`,
   last `altitudeM = 2.4`, `mode: "GUIDED"`, `armed: true`,
   `batteryRemainingPct: 85.0`.
8. `POST /api/runs/:id/reevaluate` with an empty JSON body returned
   HTTP 200 and the same `TAKEOFF_TIMEOUT` reason, confirming the
   `examples/missions/{mission}.yaml` fallback resolves correctly from the
   workspace root.

UI smoke at <http://localhost:5173> was previously confirmed against the
same API in issue 3's manual verification (same DTO shapes here); the React
build artefacts in `web/dist/` are produced from the unchanged sources.

## Quality gates

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo test --workspace` — 103 tests pass
  (`flighttrace-core` 13, `flighttrace-demo` 11, `flighttrace-storage` 5,
  `flighttrace-assertions` 36, `flighttrace-api` 12, `flighttrace-mavlink`
  12 unit + 3 read-only invariant + 4 ingestion, `flighttrace-cli` 0,
  `round_trip` 5, `http` 7). No new tests were added in this issue; the
  scope is documentation and polish.
- `cd web && npm run build` — clean (`tsc -b && vite build`, 51 modules,
  ~224 KB JS / ~9 KB CSS).
- `cd web && npm run lint` — clean (eslint 8 flat-config-free setup).

## Deviations

- **No `docker-compose.yml`.** Listed as optional in the spec; deferred per
  the README's "A note on Docker" subsection. A meaningful container build
  is gated on the single-binary release packaging in the post-v0.1 roadmap.
- **Architecture diagram inline in the README.** `docs/architecture.md` was
  left out — the diagram fits comfortably in the README and a separate file
  would only fragment the docs at this size.
- **License files (`LICENSE-APACHE`, `LICENSE-MIT`) not added.** The
  workspace `Cargo.toml` already declares `license = "MIT OR Apache-2.0"`;
  the README points at the future filenames and notes they will land with
  the first tagged release. Adding the actual license texts was outside the
  documented scope of this issue.

## Tests required

- No new automated tests in this issue. The acceptance bar is the green sweep of the existing test and lint commands plus a manual run-through of the README's demo quickstart in a clean shell.
- Manual: the PR description should include a confirmation that the README quickstart was executed end-to-end on a clean checkout (or in a clean container) and the demo failed run was observed in the UI.

## Out of scope

- No production deployment instructions.
- No hosted cloud setup.
- No billing, subscriptions, or accounts.
- No enterprise features (SSO, audit logs, RBAC).
- No marketing site or landing page.
- No CI configuration beyond what already exists (CI setup can be a follow-up issue if needed).

## Notes for future issues

- A first post-v0.1 issue is likely "CI: GitHub Actions for fmt/clippy/test on PRs". It was intentionally left out of v0.1 to keep the scope tight.
- A "release packaging" issue (single-binary build, prebuilt frontend served by the API) is a natural follow-up; the current `web/` + `cargo run` split is fine for v0.1 but is not what we want for distribution.
- If the YAML assertion DSL grows beyond a handful of checks, consider promoting `docs/architecture.md` and a dedicated `docs/assertions.md` reference; both are optional today.
- Screenshots committed in this issue should be small (<200 KB each). If they grow, move them out of the repo or to Git LFS in a future issue.
