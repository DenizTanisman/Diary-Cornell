import clsx from 'clsx';
import { useSettingsStore } from '../stores/settingsStore';
import { useTheme } from '../hooks/useTheme';
import { useT } from '../locales';
import type { Language, Theme } from '../types/settings';

export function SettingsPage() {
  const t = useT();
  const { theme, setTheme } = useTheme();
  const language = useSettingsStore((s) => s.language);
  const setLanguage = useSettingsStore((s) => s.setLanguage);

  const themeOption = (key: Theme, label: string) => (
    <button
      key={key}
      onClick={() => setTheme(key)}
      className={clsx(theme === key && 'is-active')}
      aria-pressed={theme === key}
    >
      {label}
    </button>
  );

  const langOption = (key: Language, label: string) => (
    <button
      key={key}
      onClick={() => setLanguage(key)}
      className={clsx(language === key && 'is-active')}
      aria-pressed={language === key}
    >
      {label}
    </button>
  );

  return (
    <div className="page-container">
      <h1>{t('settings.title')}</h1>

      <div className="settings-row">
        <span className="settings-row__label">{t('settings.theme')}</span>
        <div className="settings-row__control">
          {themeOption('light', t('settings.themeLight'))}
          {themeOption('dark', t('settings.themeDark'))}
          {themeOption('auto', t('settings.themeAuto'))}
        </div>
      </div>

      <div className="settings-row">
        <span className="settings-row__label">{t('settings.language')}</span>
        <div className="settings-row__control">
          {langOption('tr', t('settings.languageTR'))}
          {langOption('en', t('settings.languageEN'))}
        </div>
      </div>
    </div>
  );
}
