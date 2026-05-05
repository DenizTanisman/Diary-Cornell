import { useEffect } from 'react';
import { useSettingsStore } from '../stores/settingsStore';
import type { Theme } from '../types/settings';

// Kept in sync with themes.css --bg-primary so the Android status bar
// tint matches the app shell. If you change either side, change both.
const STATUS_BAR_COLOR = { light: '#F5F1EA', dark: '#141210' } as const;

function syncStatusBarMeta(resolved: 'light' | 'dark'): void {
  // Drop the `media` constraint while a user theme is active and pin
  // the colour to the resolved value; let the OS pick again when we
  // fall back to 'auto'.
  const tags = document.querySelectorAll<HTMLMetaElement>('meta[name="theme-color"]');
  if (!tags.length) return;
  tags.forEach((t) => t.removeAttribute('media'));
  tags[0].setAttribute('content', STATUS_BAR_COLOR[resolved]);
  // Drop any duplicates so the browser doesn't pick a stale one.
  for (let i = 1; i < tags.length; i++) tags[i].remove();
}

function restoreAutoStatusBarMeta(): void {
  // Recreate the prefers-color-scheme split if the user goes back to
  // auto. Idempotent — safe even when the tags are already correct.
  const head = document.head;
  head.querySelectorAll('meta[name="theme-color"]').forEach((t) => t.remove());
  for (const scheme of ['light', 'dark'] as const) {
    const m = document.createElement('meta');
    m.name = 'theme-color';
    m.content = STATUS_BAR_COLOR[scheme];
    m.media = `(prefers-color-scheme: ${scheme})`;
    head.appendChild(m);
  }
}

function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === 'auto') {
    const prefersDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches;
    root.dataset.theme = prefersDark ? 'dark' : 'light';
    restoreAutoStatusBarMeta();
  } else {
    root.dataset.theme = theme;
    syncStatusBarMeta(theme);
  }
}

export function useTheme(): { theme: Theme; setTheme: (t: Theme) => void } {
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);

  useEffect(() => {
    applyTheme(theme);
    if (theme !== 'auto') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => applyTheme('auto');
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme]);

  return { theme, setTheme };
}
