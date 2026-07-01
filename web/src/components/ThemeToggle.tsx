import { useCallback, useEffect, useState } from 'react';

import {
  applyTheme,
  getStoredTheme,
  nextTheme,
  setStoredTheme,
  type ThemePref,
} from '../theme/theme';

const LABEL: Record<ThemePref, string> = {
  system: 'auto',
  light: 'light',
  dark: 'dark',
};

export function ThemeToggle() {
  const [pref, setPref] = useState<ThemePref>('system');

  // Initial mount: sync UI state with whatever the inline pre-paint
  // script + tokens.css already picked, and start tracking system
  // changes when the user is in `'system'` mode.
  useEffect(() => {
    setPref(getStoredTheme());
  }, []);

  const onClick = useCallback(() => {
    setPref((current) => {
      const next = nextTheme(current);
      applyTheme(next);
      setStoredTheme(next);
      return next;
    });
  }, []);

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={onClick}
      aria-label={`Theme: ${LABEL[pref]}. Click to change.`}
      title={`Theme: ${LABEL[pref]}`}
    >
      <span className="theme-toggle-glyph" aria-hidden="true">
        {renderGlyph(pref)}
      </span>
      <span>{LABEL[pref]}</span>
    </button>
  );
}

function renderGlyph(pref: ThemePref) {
  // Inline SVG so no icon library required. Filled shapes so the glyph
  // reads on both warm cream and warm charcoal backgrounds.
  switch (pref) {
    case 'light':
      return (
        <svg viewBox="0 0 16 16" width="14" height="14">
          <circle cx="8" cy="8" r="3.4" fill="currentColor" />
          {[0, 45, 90, 135, 180, 225, 270, 315].map((deg) => {
            const rad = (deg * Math.PI) / 180;
            const x1 = 8 + Math.cos(rad) * 5;
            const y1 = 8 + Math.sin(rad) * 5;
            const x2 = 8 + Math.cos(rad) * 7;
            const y2 = 8 + Math.sin(rad) * 7;
            return (
              <line
                key={deg}
                x1={x1}
                y1={y1}
                x2={x2}
                y2={y2}
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
              />
            );
          })}
        </svg>
      );
    case 'dark':
      return (
        <svg viewBox="0 0 16 16" width="14" height="14">
          <path
            d="M11.5 10.2A5.5 5.5 0 0 1 5.8 4.5c0-.4 0-.8.1-1.2A6 6 0 1 0 12.7 10a5.4 5.4 0 0 1-1.2.2z"
            fill="currentColor"
          />
        </svg>
      );
    default:
      // system: half-lit disc — hints at "follows OS".
      return (
        <svg viewBox="0 0 16 16" width="14" height="14">
          <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
          <path d="M8 2.5a5.5 5.5 0 0 1 0 11z" fill="currentColor" />
        </svg>
      );
  }
}
