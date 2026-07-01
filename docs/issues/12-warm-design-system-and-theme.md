# Issue 12 — Warm design system and light/dark theme switch

## Summary

Restyle FlightTrace to match the warm desktop-tool family (MTX Catalog
Explorer / MTX Service Inspector): parchment/warm-cream in light mode,
warm charcoal in dark mode, terracotta as the primary accent, sage
green as the secondary accent. Add a persistent light/dark toggle in
the app header.

Frontend/UI only. No backend changes, no API changes, no new product
features. The dense mono-heavy debugger character stays exactly as it
is — this is purely a palette, typography, and theme-runtime pass on
the existing structure.

## Product goal

- FlightTrace visually belongs to the MTX desktop-tool family: warm
  parchment or warm charcoal surfaces, terracotta accents, serif
  headings, mono for data.
- The failure summary card remains the strongest visual section — it
  still stops the eye first.
- A theme toggle in the header switches between light and dark,
  persists to `localStorage`, and defaults to `prefers-color-scheme`
  on first visit.
- Existing components (chart, timeline, tables, raw event log)
  restyle from the new tokens automatically; no per-component visual
  rewrites.

## Technical scope

1. **`web/src/styles/tokens.css`** — full rewrite:
   - Root defaults resolved from `prefers-color-scheme`.
   - `[data-theme="light"]` and `[data-theme="dark"]` overrides.
   - New token names per spec: `--bg / --bg-2 / --bg-3`,
     `--fg / --fg-2 / --fg-3`, `--accent / --accent-strong`,
     `--accent-2 / --accent-2-strong` (sage secondary),
     `--danger / --warning / --success`, `--hl-bg`, `--border /
     --border-strong`.
   - Per-tone tinted backgrounds — `--danger-bg`, `--warning-bg`,
     `--success-bg`, `--neutral-bg` — defined explicitly per theme so
     switching doesn't need `color-mix()` support at runtime.
   - New font-family tokens: `--font-display`, `--font-body`,
     `--font-mono`, all with safe system fallbacks. No network font
     loading in v0.1 — self-hosted Fraunces / Inter Tight / Inconsolata
     is a follow-up.
   - `--radius-pill: 999px` for badges and toggle chips.
   - `--transition-fast: 140ms ease` for hover / focus.

2. **`web/src/styles/globals.css`** — sweep all consumers to the new
   token names:
   - `--surface` → `--bg-2`
   - `--surface-raised` → `--bg-3`
   - `--surface-elevated` → `--bg-3`
   - `--text` / `--text-muted` / `--text-dim` → `--fg` / `--fg-2` / `--fg-3`
   - `--link` → `--accent`
   - `--neutral` → `--fg-2`
   - `--accent-bg` → `--hl-bg`
   - `--font-sans` → `--font-body`
   - `--mono` → `--font-mono`
   - Body font applied at `body`; display font applied to `.brand h1`
     and section-header `h2`.
   - Pill radius on `.badge` and the new theme toggle.
   - 140ms transitions on hover/focus states.

3. **`web/src/theme/theme.ts`** — small runtime helper:
   - `ThemePref = 'light' | 'dark' | 'system'`.
   - `getStoredTheme()` reads `localStorage.flighttrace-theme`.
   - `resolveTheme(pref)` collapses `'system'` to the current
     `prefers-color-scheme` value.
   - `applyTheme(theme)` sets `document.documentElement.dataset.theme`.
   - `subscribeSystem(cb)` watches the media query so `'system'` mode
     tracks OS changes live.

4. **`web/src/components/ThemeToggle.tsx`** — a small header button:
   - Three states cycle: `system` → `light` → `dark` → `system`.
   - Renders a sun/moon/circle glyph reflecting the current state.
   - Writes to `localStorage` and calls `applyTheme` on click.

5. **`web/src/App.tsx`** — mounts the toggle in the header,
   right-aligned next to the existing `local · v0.1 · demo only` chip.

6. **`web/index.html`** — inline synchronous script (before
   `main.tsx` loads) that:
   - Reads `localStorage.flighttrace-theme`.
   - Falls back to `prefers-color-scheme` if unset.
   - Sets `document.documentElement.dataset.theme` before any styles
     paint. Zero flash of wrong theme.
   - Also drops the hard-coded `<meta name="color-scheme">` in favour
     of a value that reflects the current theme, so browser scrollbars
     etc. match.

7. **Docs**: brief mention of the theme switch in
   `README.md`, `docs/desktop.md`, and update the palette section of
   `docs/tech-stack.md` to reflect the new warm tokens. Note the
   self-hosted-fonts follow-up.

## Non-goals

- **No font hosting.** We use `--font-display` / `--font-body` /
  `--font-mono` with system fallbacks. Self-hosting Fraunces / Inter
  Tight / Inconsolata is a follow-up issue.
- **No new components.** Restyling only, plus one small
  `ThemeToggle`.
- **No React state library**, no context provider. The theme runtime
  is 50 lines of vanilla TS operating on `document.documentElement`.

## Acceptance

- [x] Light mode reads as warm parchment (`--bg #FAFAF8`), not pure
  white — `web/src/styles/tokens.css`.
- [x] Dark mode reads as warm charcoal (`--bg #1A1816`), not cold
  navy — same file.
- [x] Terracotta is the primary accent in both themes; sage is the
  secondary accent / OK state — `--accent`, `--accent-2`, `--success`.
- [x] Header toggle cycles system → light → dark → system —
  `nextTheme` in `web/src/theme/theme.ts`.
- [x] Selection persists across reloads — `localStorage` key
  `flighttrace-theme`.
- [x] First-visit default follows `prefers-color-scheme` — the
  `:root:not([data-theme])` block in `tokens.css`.
- [x] No flash of the wrong theme on page load — synchronous inline
  script in `web/index.html` sets `data-theme` before styles paint.
- [x] Failure summary card remains the strongest section in both
  themes — `--danger` left border + `linear-gradient(--danger-bg, --bg-2)`
  background survives the sweep unchanged.
- [x] Altitude chart and raw event log stay readable — all their
  refs went through the mechanical rename and now consume warm
  tokens.
- [x] Existing packaged web mode still works — same
  `flighttrace serve --web-dir …` binary; no backend changes.
- [x] Existing Tauri desktop mode still works — same crate; verified
  via `cargo check` + `cargo clippy` below.
- [x] `cargo fmt --all -- --check` green.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` green.
- [x] `cargo test --workspace` green (19 `test result: ok` lines).
- [x] `cd web && npm run build && npm run lint` green — bundle grew
  0.09 kB CSS + 0.20 kB JS for the toggle + tokens.
- [x] `cd apps/flighttrace-desktop/src-tauri && cargo check` green.
- [x] `cd apps/flighttrace-desktop/src-tauri && cargo clippy --all-targets -- -D warnings` green.

## Verification

- Workspace + web + desktop gates all green.
- `tokens.css` rewrite: dropped from 45 tokens to 40, with cleaner
  semantic names (`--bg-2` / `--bg-3` / `--fg` / `--fg-2` / `--fg-3`
  vs. the old `--surface-*` / `--text-*` set). `[data-theme='dark']`
  override + `prefers-color-scheme` `:root:not([data-theme])` block
  handle the two-way switch.
- `globals.css` sweep: 20+ old token names replaced via `sed`; zero
  residual old references (`grep -c` confirmed).
- Typography: `--font-body` on body; `--font-display` on `.app-header
  h1` (18 px, weight 600) and `.run-detail-header h2` (22 px);
  `--font-mono` unchanged on data cells.
- Pill radius on `.badge`; 140 ms transitions on `.button` and
  `.theme-toggle`; warm scrollbar via `::-webkit-scrollbar-thumb` +
  `scrollbar-color`; hardcoded `#93c2ff` primary-button hover swapped
  to `--accent-strong`.
- Theme toggle: three-state cycle (`system` / `light` / `dark`), sun /
  moon / half-disc glyphs, sits in the app-header right slot next to
  the `local · v0.1 · demo only` chip.

## Deviations

- **No hosted fonts.** Fraunces / Inter Tight / Inconsolata are named
  first in the font stacks with system fallbacks after. Self-hosting
  them (or wiring a font-download build step) is a follow-up.
- **No documented screenshots refresh.** The `docs/screenshots/*.png`
  files are still the palette-matched placeholders from issue 11; they
  now visibly mismatch the warm palette. Regenerating them (either the
  placeholders or real captures) is queued for issue 13.
- **Old `--radius-lg` unused** but retained in `tokens.css` in case a
  future card variant needs it. `--space-8` is likewise defined and
  currently unused.
