// Tiny theme runtime. Keeps state on <html data-theme> so tokens.css can
// pick it up. Persists the user's explicit choice in localStorage under
// STORAGE_KEY; when the user picks "system", we clear the key and let the
// OS-preference media query in tokens.css do the work.
//
// A small twin of this logic is inlined into `web/index.html` as a
// synchronous script so the theme applies before styles paint. If you
// touch STORAGE_KEY or the value shape, update index.html in lockstep.

export type ThemePref = 'light' | 'dark' | 'system';

export const STORAGE_KEY = 'flighttrace-theme';

export function getStoredTheme(): ThemePref {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === 'light' || raw === 'dark' || raw === 'system') return raw;
  } catch {
    /* localStorage unavailable — fall through to system. */
  }
  return 'system';
}

export function resolveTheme(pref: ThemePref): 'light' | 'dark' {
  if (pref === 'light' || pref === 'dark') return pref;
  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  return mq.matches ? 'dark' : 'light';
}

/**
 * Apply a preference to the DOM. For `'system'`, the attribute is
 * removed so tokens.css's `prefers-color-scheme` fallback takes over
 * and continues to track OS changes live.
 */
export function applyTheme(pref: ThemePref): void {
  const html = document.documentElement;
  if (pref === 'system') {
    delete html.dataset.theme;
  } else {
    html.dataset.theme = pref;
  }
}

/**
 * Persist a preference. `'system'` clears the key rather than storing
 * the string so a future release can change the default without
 * silently overriding an intentional user choice.
 */
export function setStoredTheme(pref: ThemePref): void {
  try {
    if (pref === 'system') {
      window.localStorage.removeItem(STORAGE_KEY);
    } else {
      window.localStorage.setItem(STORAGE_KEY, pref);
    }
  } catch {
    /* ignore; theme still applies via applyTheme. */
  }
}

/** Cycle order used by the header toggle. */
export function nextTheme(current: ThemePref): ThemePref {
  return current === 'system' ? 'light' : current === 'light' ? 'dark' : 'system';
}
