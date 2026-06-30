# Release checklist

A working checklist for cutting a FlightTrace release. Copy the relevant
sections into the release PR or GitHub release draft.

The default branch is `main`. Releases follow [Semantic Versioning] with
`v<major>.<minor>.<patch>` git tags (`v0.1.0`, `v0.2.0`, `v0.1.1`).

[Semantic Versioning]: https://semver.org/

---

## 1. Demo quickstart verification

Run on a clean clone (or fresh container) with no `./data/` directory. Two
terminals, exactly as written in [`README.md`](../README.md).

- [ ] `git clone <repo> && cd FlightTrace`
- [ ] Terminal 1: `cargo run -p flighttrace-cli -- serve --bind 127.0.0.1:8080 --database ./data/flighttrace.db` — API binds, `INFO FlightTrace API listening` appears in the logs.
- [ ] `curl -fsS http://127.0.0.1:8080/health` returns `{"status":"ok","version":"<crate version>"}`.
- [ ] Terminal 2: `cd web && npm install && npm run dev` — Vite reports `Local: http://localhost:5173/`.
- [ ] Browser at <http://localhost:5173> renders the runs list with no errors in the dev console.
- [ ] Click **Create demo failed run** — UI navigates to the new run's detail page.
- [ ] Run detail page shows: status `FAILED — TAKEOFF_TIMEOUT` badge, failure report card (`Mission failed: TAKEOFF_TIMEOUT`, evidence list with at least one `Statustext`, likely-cause paragraph), mission timeline (`CONNECTED` → `ARMED` → `TAKEOFF_REQUESTED` → `CLIMBING` → `FAILED`), assertion table with at least one `FAIL` row (`takeoff.altitude_at_least_m`), telemetry overview (altitude rising from 0.0 to ~2.4 m), and raw event log.
- [ ] Refresh the page — all panels re-populate from the API; no client-only state is lost.
- [ ] Stop both processes with Ctrl+C — server logs `shutdown signal received` and exits cleanly.

## 2. SITL smoke (recommended, not blocking)

Skip if no SITL is available, but record the result in the release notes.

- [ ] Start PX4 SITL or ArduPilot SITL on `udp://127.0.0.1:14550` per the README.
- [ ] Start FlightTrace with `--mavlink udp://127.0.0.1:14550 --mission examples/missions/simple_takeoff.yaml`.
- [ ] A new run with `source = mavlink` appears in the UI within a few seconds.
- [ ] After arming + taking off, the run accumulates `HeartbeatReceived`, `ModeChanged`, `ArmedChanged`, and `AltitudeUpdated` events.
- [ ] Stop SITL; after the inactivity timeout, the run is finalised with assertion results and (on failure) a `FailureReport`.

## 3. Screenshot checklist

Drop fresh PNGs in `docs/screenshots/`. Reference them from the README only
once they are committed. Keep each file under ~200 KB.

- [ ] `docs/screenshots/runs-list.png` — runs list with the failed demo run at the top (badge visible).
- [ ] `docs/screenshots/run-detail.png` — run detail page with the failure report card visible.
- [ ] `docs/screenshots/failure-report.png` — close-up of the failure report card with evidence + likely cause.
- [ ] Browser window sized to 1440×900.
- [ ] Dark theme (default — do not override).
- [ ] No personal identifiers, real-vehicle telemetry, or unredacted local paths visible.

## 4. Pre-tag quality bar

All of these must pass locally and in CI before tagging.

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cd web && npm ci && npm run build && npm run lint`
- [ ] CI on `main` is green for the commit you intend to tag.
- [ ] No new dependency on a banned tech listed in [`docs/context.md`](context.md) §7.
- [ ] `README.md`, `docs/prd.md`, and `docs/context.md` reflect the actual behaviour of the code being released.

## 5. Release tag checklist

- [ ] Decide the version. Bump per SemVer:
  - **major** if any public API in `flighttrace-core`, the HTTP API, or the YAML mission DSL changed in an incompatible way.
  - **minor** for additive features (new endpoints, new YAML checks, new CLI flags, new normalized event kinds).
  - **patch** for fixes that change no public surface.
- [ ] Update `version = "<x.y.z>"` in the workspace `Cargo.toml` (`[workspace.package]`). All crates inherit this — do **not** edit per-crate versions.
- [ ] Run `cargo build --workspace --locked` so `Cargo.lock` is refreshed against the new version.
- [ ] Update `CHANGELOG.md` (or the release notes draft) with the user-visible changes since the last tag — group as Added / Changed / Fixed / Removed.
- [ ] Confirm the per-issue docs under `docs/issues/` for everything included in this release have ticked acceptance criteria and verification sections.
- [ ] Open a release PR titled `chore(release): v<x.y.z>` containing the version bump, the changelog entry, and any last-mile doc updates.
- [ ] After the release PR merges, tag the merge commit:
  ```bash
  git checkout main && git pull --ff-only
  git tag -a v<x.y.z> -m "FlightTrace v<x.y.z>"
  git push origin v<x.y.z>
  ```
- [ ] Create the GitHub release from the tag using the template in §6 below. Attach committed screenshots as inline images, not release assets.
- [ ] Announce in the project README (if applicable) or the team's usual channel.

## 6. GitHub release notes template

Copy this into the GitHub release editor and fill in the bracketed fields.

```markdown
# FlightTrace v<x.y.z>

<one-line tagline — e.g. "First public release of the local mission debugger.">

## Highlights
- <bullet — the headline change a user would care about>
- <bullet — secondary headline change>
- <bullet — anything noteworthy about scope or compatibility>

## Added
- <new feature> (#<issue or PR>)

## Changed
- <behaviour change a user will notice> (#<issue or PR>)

## Fixed
- <bug fix that resolves a reported issue> (#<issue or PR>)

## Removed
- <only when something was deliberately taken out>

## Compatibility
- Rust: stable toolchain (edition 2024, MSRV ≥ 1.85).
- Node.js: ≥ 18 for the web UI.
- Storage: SQLite database file from the previous release is forward-compatible; back up `./data/flighttrace.db` before upgrading.
- HTTP API: <"no breaking changes" | "breaking change: <field> was renamed">.

## Verification
- [x] Demo quickstart from the README passes end-to-end (`POST /api/demo/run` → `TAKEOFF_TIMEOUT` failure report rendered in the UI).
- [x] `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `npm run build`, `npm run lint` all green on CI for the tagged commit.
- [<x|->] SITL smoke against <PX4 | ArduPilot> SITL.

## Known limitations
- <if any — e.g. "Live UI streaming is not implemented; refresh the page to see new events from a MAVLink session.">

## Acknowledgements
Thanks to <names / handles> for contributions in this release.
```

---

## Notes

- This checklist is intentionally short and copy-pasteable. It is not a CI
  config — it is a human checklist for the release driver.
- The first post-v0.1 release will likely include the CI workflow and the
  license files added in the release-hardening sweep; both are already in
  `main` and tested before this checklist is exercised.
- A `CHANGELOG.md` does not yet exist in v0.1. Create it as part of the
  first release that uses this checklist; subsequent releases only append a
  new section.
