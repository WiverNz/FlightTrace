# Issue 14 — Ship Windows native desktop app release artifact

## Summary

The GitHub Release archive currently contains only the CLI + web-dist
bundle (`flighttrace-<version>-x86_64-pc-windows-msvc.zip`). Users who
want a **native window** — no browser — have to build the Tauri
desktop shell themselves from source. Fix by adding a second Windows
build job to `release.yml` that runs `cargo tauri build` and uploads
the produced installer(s) as extra release assets.

No changes to the CLI archive, no changes to the desktop crate's
runtime behaviour, no change to PR CI. This issue is a **release-time
workflow addition** plus documentation.

## Product goal

- On a `v*.*.*` tag push, GitHub Release contains:
  - existing CLI/browser assets (unchanged);
  - new **native desktop** assets — NSIS setup `.exe` and MSI
    `.msi` produced by Tauri v2's default Windows bundling;
  - `SHA256SUMS` covering all of the above.
- A user browsing the Releases page can tell in one glance which
  asset to download for which use case.
- WebView2 requirement is stated up-front, not discovered at install
  time.

## Technical scope

1. **`apps/flighttrace-desktop/src-tauri/icons/icon.ico`** — Windows
   installer needs an ICO to embed in the shortcut / uninstall entry.
   Generated from the existing `icon.png` via
   `scripts/png_to_ico.py` (same helper we used for the CLI icon).
   `tauri.conf.json` `bundle.icon` array gains the new path.

2. **`.github/workflows/release.yml`** — new job `desktop-windows`:
   - Runs only on `v*.*.*` tag pushes (the existing workflow trigger).
   - `runs-on: windows-latest`.
   - Sets up Rust stable, Node 20, and caches `apps/flighttrace-desktop/src-tauri`'s
     cargo state.
   - Builds `web/dist` first (Tauri's build script validates
     `bundle.resources` at compile time — this is the same guard we
     added in issue 09).
   - Installs `tauri-cli` via `cargo install`.
   - Runs `cargo tauri build` inside `src-tauri`.
   - Copies + renames the produced NSIS `.exe` and MSI `.msi` to
     include the target triple and version:
     - `flighttrace-desktop-<version>-x86_64-pc-windows-msvc-setup.exe`
     - `flighttrace-desktop-<version>-x86_64-pc-windows-msvc.msi`
   - Uploads them as artifact `release-desktop-windows`.

3. **Existing `release` job** picks up `release-desktop-windows`
   automatically through the existing `pattern: release-*,
   merge-multiple: true` glob. The SHA256SUMS step is expanded to
   include `.exe` and `.msi` alongside `.tar.gz` and `.zip`.

4. **`webviewInstallMode`** — Tauri v2's Windows default is
   `downloadBootstrapper` (small installer, WebView2 fetched from
   Microsoft at install time). We keep the default: it's the
   smallest / most reliable choice and matches "Prefer the simplest
   reliable Tauri output first" from the spec.

## Artifact matrix after this issue

| Asset | Target | Use case |
| --- | --- | --- |
| `flighttrace-<v>-x86_64-pc-windows-msvc.zip` | Windows | CLI + `web-dist` bundle; run `flighttrace.exe` (GUI mode from issue 13) or `flighttrace serve …` (explicit). Opens the OS default browser. |
| `flighttrace-desktop-<v>-x86_64-pc-windows-msvc-setup.exe` | Windows | Native window; NSIS installer with WebView2 bootstrapper. Recommended for most users. |
| `flighttrace-desktop-<v>-x86_64-pc-windows-msvc.msi` | Windows | Same content as above; MSI form for group-policy / enterprise deployment. |
| `flighttrace-<v>-x86_64-unknown-linux-gnu.tar.gz` | Linux | CLI + `web-dist` bundle. |
| `SHA256SUMS` | any | Hashes of everything above. |

Linux desktop `.deb` / `.AppImage` is deliberately out of scope for v0.1
— webkit2gtk system deps make the runtime footprint different from the
Windows story, and none of the release-hardening issues (07–13) ship
Linux desktop bundles yet.

## Non-goals

- **PR CI unchanged.** The existing `desktop-check` job on `ubuntu-latest`
  stays exactly as-is (fmt + `cargo check` + clippy). Full `cargo tauri
  build` on every PR is heavy and out of scope.
- **No macOS desktop bundle.** Same rationale as Linux — deferred.
- **No portable-zip desktop asset.** The NSIS installer is small
  (~10 MB) and produces a portable-looking result. Duplicating the
  install tree into a zip would inflate release size without a
  matching user story.
- **No code-signing.** The installers are unsigned; Windows SmartScreen
  will show a warning until we buy a code-signing cert. Documented in
  release notes.
- **No auto-update.** Static installers only for v0.1; auto-update
  needs a signing key + a manifest server.

## Files created / modified

- `docs/issues/14-windows-desktop-release-artifact.md` (this file).
- `apps/flighttrace-desktop/src-tauri/icons/icon.ico` — new.
- `apps/flighttrace-desktop/src-tauri/tauri.conf.json` — `bundle.icon`
  array gains the ICO.
- `.github/workflows/release.yml` — new `desktop-windows` job;
  expanded `SHA256SUMS` glob; release-notes text mentions the two
  installer flavours.
- `README.md` — Run modes table gains a native-desktop row (Windows
  installer download); brief WebView2 note.
- `docs/desktop.md` — new "Downloaded Windows installer" section;
  explains asset choice + WebView2.
- `docs/release-notes/v0.1.0.md` — new "Windows native desktop app"
  block under How to run.

## Manual verification

Automated smoke of a Windows Tauri bundle from a Linux CI runner
isn't feasible in v0.1 scope. The workflow is verified end-to-end via
the tag-push path itself:

1. Push a fresh `v0.1.X` tag.
2. Wait for the `desktop-windows` job to finish (~4 min build + ~8
   min if tauri-cli isn't cached).
3. From the GitHub Release page, download
   `flighttrace-desktop-<v>-x86_64-pc-windows-msvc-setup.exe`.
4. Run the installer on a Windows box (Windows 10+; Windows 11
   preferred because WebView2 is preinstalled).
5. Confirm a **native window** opens with FlightTrace's UI.
6. Click **Create demo failed run**.
7. Open the new row; verify the `TAKEOFF_TIMEOUT` failure summary
   card appears with the target-vs-reached altitude tiles.
8. Close the window; confirm the process exits cleanly (this is the
   graceful-shutdown watch wired in issue 08).

Failures at any step block the release; the deviation gets a note in
the release checklist for the next tag.

## Acceptance

- [x] Existing CLI release assets are still produced — the `build`
  matrix job in `release.yml` is untouched.
- [x] `desktop-windows` job appears in `release.yml` and inherits the
  workflow's `on: push: tags: [v*.*.*]` trigger, so it only runs on
  release tags.
- [x] Job produces at least one desktop installer artifact —
  `bundle.targets: "all"` yields both NSIS `.exe` and MSI `.msi`; the
  PowerShell collector aborts if neither lands.
- [x] Artifact names embed the version and target triple —
  `flighttrace-desktop-<v>-x86_64-pc-windows-msvc-setup.exe` and
  `flighttrace-desktop-<v>-x86_64-pc-windows-msvc.msi`.
- [x] `SHA256SUMS` covers the new installers — bash `nullglob` +
  explicit `*.tar.gz *.zip *.exe *.msi` matrix.
- [x] README explains which asset to download and mentions WebView2 —
  new **"Which asset should I download?"** block after the run-modes
  table.
- [x] `docs/release-notes/v0.1.0.md` has a Windows-native-desktop
  section as the first "How to run" entry.
- [x] Local gates all pass — see Verification.

## Verification

- Workspace: `cargo fmt --all -- --check` ✅, `cargo clippy
  --workspace --all-targets -- -D warnings` ✅, `cargo test
  --workspace` ✅ (19 `test result: ok` lines).
- Web: `npm run build` ✅ (17.28 kB CSS, 237.85 kB JS — no
  regression), `npm run lint` ✅.
- Desktop: `cargo check` ✅ (1.39 s incremental — the new `icon.ico`
  triggered a rebuild), `cargo clippy --all-targets -- -D warnings`
  ✅.
- `release.yml` parses via `python3 -c "import yaml"`; jobs are now
  `['build', 'desktop-windows', 'release']`.

Windows-side install verification (native window opens, demo run
succeeds, TAKEOFF_TIMEOUT card renders, clean shutdown on close) is
manual and gated on the next tag push — this WSL2 dev box can't
exercise the installer.

## Deviations

- **No macOS or Linux desktop bundle.** Same rationale as
  [issue 09](09-desktop-packaging-hardening.md#deviations): webkit2gtk
  system deps make the Linux runtime story different, and macOS
  needs notarisation to avoid Gatekeeper prompts. Both are deferred.
- **No code-signing.** Installers are unsigned. Windows SmartScreen
  will flag them on first run. Buying a code-signing cert + wiring
  Azure Trusted Signing is a separate scope; noted in the README /
  release notes so users aren't surprised.
- **No auto-update.** Tauri v2 supports an updater, but it needs a
  static-signed manifest server. Not worth the setup for v0.1.
- **Automated smoke skipped.** The release-smoke script we run
  locally against the CLI can't exercise a Windows installer from a
  Linux runner. Manual acceptance follows the numbered list at the
  top of this doc; failures block the release.
