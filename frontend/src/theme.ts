/**
 * Theme manager for the React frontend (#2071).
 *
 * Mirrors the dark/light toggle logic in backend/simulator.html (around the
 * `initTheme` / `toggleTheme` / `updateToggleButton` helpers):
 *   - The explicit choice is persisted in localStorage under the "ttl-theme"
 *     key with values "light" | "dark".
 *   - That choice is reflected on <html> as the data-theme attribute, which
 *     the tokens in styles/tokens.css react to.
 *   - When no explicit choice is stored, the OS prefers-color-scheme is
 *     honoured (data-theme is left unset so the CSS media query applies).
 *   - If the OS theme changes while no explicit choice has been made, the
 *     displayed state re-syncs (the toggle label updates), but data-theme is
 *     not forced so the CSS media query keeps driving the palette.
 */

import { useCallback, useEffect, useState } from 'react';

export type Theme = 'light' | 'dark';

const STORAGE_KEY = 'ttl-theme';

function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || !window.matchMedia) return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function readStoredTheme(): Theme | null {
  if (typeof window === 'undefined') return null;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored === 'light' || stored === 'dark' ? stored : null;
}

/**
 * The theme currently in effect: the stored choice, if any, otherwise the OS
 * preference (defaulting to dark when the OS reports light or is unavailable,
 * matching the dark-first :root defaults in tokens.css).
 */
export function getTheme(): Theme {
  const stored = readStoredTheme();
  if (stored) return stored;
  return systemPrefersDark() ? 'dark' : 'light';
}

/**
 * Persist an explicit theme choice and reflect it on <html data-theme>.
 * Use this for imperative (non-React) callers; React components should prefer
 * the useTheme() hook so they re-render on change.
 */
export function setTheme(theme: Theme): void {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', theme);
  }
  if (typeof window !== 'undefined') {
    window.localStorage.setItem(STORAGE_KEY, theme);
  }
}

/**
 * Flip between light and dark, persisting the new choice. Returns the theme
 * that is now in effect.
 */
export function toggleTheme(): Theme {
  const next: Theme = getTheme() === 'dark' ? 'light' : 'dark';
  setTheme(next);
  return next;
}

export interface UseThemeResult {
  /** The theme currently in effect (stored choice or OS default). */
  theme: Theme;
  /** Flip between light and dark, persisting the choice. */
  toggle: () => void;
  /** Explicitly set and persist a theme. */
  setTheme: (theme: Theme) => void;
}

/**
 * React hook backing the dark/light theme toggle (#2071).
 *
 * On mount it restores the saved preference (or falls back to the OS
 * preference) and applies data-theme only when an explicit choice exists,
 * leaving it unset otherwise so the tokens.css media query can drive the
 * palette. It re-syncs the displayed state when the OS preference changes
 * and no explicit choice has been made.
 */
export function useTheme(): UseThemeResult {
  const [theme, setThemeState] = useState<Theme>(getTheme);

  // Apply/restore the data-theme attribute once on mount. Mirrors the
  // simulator initTheme(): set the attribute only when there is a stored
  // choice; otherwise remove it so the prefers-color-scheme media query in
  // tokens.css decides the active palette.
  useEffect(() => {
    const stored = readStoredTheme();
    if (stored) {
      document.documentElement.setAttribute('data-theme', stored);
    } else {
      document.documentElement.removeAttribute('data-theme');
    }
  }, []);

  // Re-sync the displayed state when the OS preference changes and the user
  // has not made an explicit choice. Mirrors the simulator's matchMedia
  // 'change' listener, which only refreshes the toggle label.
  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return;
    const mql = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (!readStoredTheme()) {
        setThemeState(systemPrefersDark() ? 'dark' : 'light');
      }
    };
    mql.addEventListener('change', onChange);
    return () => mql.removeEventListener('change', onChange);
  }, []);

  const toggle = useCallback(() => {
    setThemeState((current) => {
      const next: Theme = current === 'dark' ? 'light' : 'dark';
      // Persist + apply the explicit choice (mirrors simulator toggleTheme).
      window.localStorage.setItem(STORAGE_KEY, next);
      document.documentElement.setAttribute('data-theme', next);
      return next;
    });
  }, []);

  const handleSetTheme = useCallback((next: Theme) => {
    window.localStorage.setItem(STORAGE_KEY, next);
    document.documentElement.setAttribute('data-theme', next);
    setThemeState(next);
  }, []);

  return { theme, toggle, setTheme: handleSetTheme };
}

export default useTheme;
