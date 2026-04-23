import { create } from 'zustand';
import type { Language, Theme } from '../types/settings';

interface SettingsState {
  theme: Theme;
  language: Language;
  autoSaveIntervalMs: number;
  setTheme: (theme: Theme) => void;
  setLanguage: (lang: Language) => void;
  setAutoSaveInterval: (ms: number) => void;
  hydrate: (partial: Partial<Omit<SettingsState, 'setTheme' | 'setLanguage' | 'setAutoSaveInterval' | 'hydrate'>>) => void;
}

const THEME_KEY = 'cornell-diary:theme';
const storedTheme = (typeof localStorage !== 'undefined'
  ? (localStorage.getItem(THEME_KEY) as Theme | null)
  : null) ?? 'auto';

export const useSettingsStore = create<SettingsState>((set) => ({
  theme: storedTheme,
  language: 'tr',
  autoSaveIntervalMs: 1500,
  setTheme: (theme) => {
    if (typeof localStorage !== 'undefined') localStorage.setItem(THEME_KEY, theme);
    set({ theme });
  },
  setLanguage: (language) => set({ language }),
  setAutoSaveInterval: (autoSaveIntervalMs) => set({ autoSaveIntervalMs }),
  hydrate: (partial) => set(partial),
}));
