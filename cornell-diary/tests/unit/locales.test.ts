import { beforeEach, describe, expect, it } from 'vitest';
import { t } from '../../src/locales';
import { useSettingsStore } from '../../src/stores/settingsStore';

beforeEach(() => {
  useSettingsStore.setState({ language: 'tr' });
});

describe('i18n t()', () => {
  it('returns Turkish strings by default', () => {
    expect(t('nav.today')).toBe('Bugün');
    expect(t('sync.exportTitle')).toMatch(/Dışa/);
  });

  it('switches to English when language changes', () => {
    useSettingsStore.setState({ language: 'en' });
    expect(t('nav.today')).toBe('Today');
  });

  it('interpolates variables', () => {
    expect(t('diary.wordCount', { count: 42 })).toContain('42');
  });

  it('returns key on miss', () => {
    // @ts-expect-error intentional bad key
    expect(t('does.not.exist')).toBe('does.not.exist');
  });
});
