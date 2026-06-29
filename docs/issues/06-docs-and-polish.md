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

- [ ] README contains: pitch, what-it-is-not, architecture diagram, demo quickstart, SITL quickstart (PX4 and ArduPilot), project layout, mission-assertions section, troubleshooting, and a short roadmap.
- [ ] README demo quickstart, copy-pasted as-is into a fresh shell, produces a visible `TAKEOFF_TIMEOUT` failure report in the browser.
- [ ] `examples/missions/simple_takeoff.yaml` exists and is referenced from the README.
- [ ] `docs/screenshots/` directory exists with a `.gitkeep`, and the README explains how to capture screenshots.
- [ ] Optional `docker-compose.yml` is either present and documented, or its absence is intentional and noted.
- [ ] `.gitignore` excludes `target/`, `web/node_modules`, `web/dist`, `data/`, and local env files.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] `npm run build` and `npm run lint` in `web/` pass.
- [ ] `docs/prd.md` section 12 has all v0.1 acceptance checkboxes ticked, reflecting the actual implementation state.

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
