export type Theme = 'light' | 'dark' | 'auto';
export type Language = 'tr' | 'en';

export interface AppSettings {
  theme: Theme;
  language: Language;
  autoSaveIntervalMs: number;
  firstLaunchDate: string;
}

export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'auto',
  language: 'tr',
  autoSaveIntervalMs: 1500,
  firstLaunchDate: new Date().toISOString(),
};
