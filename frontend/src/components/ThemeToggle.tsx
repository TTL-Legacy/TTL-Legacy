import React from 'react';
import { useTheme } from '../theme';

/**
 * Accessible dark/light theme toggle button (#2071).
 *
 * Backed by the useTheme() hook in frontend/src/theme.ts, which mirrors the
 * "ttl-theme" localStorage key and data-theme attribute used by
 * backend/simulator.html. Styled entirely with design-system tokens from
 * styles/tokens.css. When the app is in dark mode the button offers to switch
 * to light (and vice versa), matching the simulator's icon/label convention.
 */
export function ThemeToggle() {
  const { theme, toggle } = useTheme();
  const isDark = theme === 'dark';

  return (
    <button
      type="button"
      onClick={toggle}
      aria-label={isDark ? 'Switch to light theme' : 'Switch to dark theme'}
      aria-pressed={isDark}
      data-testid="theme-toggle"
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 6,
        padding: '0.4rem 0.75rem',
        background: 'var(--color-surface)',
        color: 'var(--color-text-primary)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius)',
        cursor: 'pointer',
        fontFamily: 'inherit',
        fontSize: '0.9rem',
      }}
    >
      <span aria-hidden="true">{isDark ? '☀️' : '🌙'}</span>
      <span>{isDark ? 'Light' : 'Dark'}</span>
    </button>
  );
}

export default ThemeToggle;
