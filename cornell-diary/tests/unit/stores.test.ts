import { beforeEach, describe, expect, it } from 'vitest';
import { useSettingsStore } from '../../src/stores/settingsStore';
import { useSyncStore } from '../../src/stores/syncStore';

beforeEach(() => {
  useSettingsStore.setState({ theme: 'auto', language: 'tr', autoSaveIntervalMs: 1500 });
  useSyncStore.setState({ dialog: 'none', lastResult: null });
  if (typeof localStorage !== 'undefined') localStorage.clear();
});

describe('settingsStore', () => {
  it('updates theme and persists to localStorage', () => {
    useSettingsStore.getState().setTheme('dark');
    expect(useSettingsStore.getState().theme).toBe('dark');
    expect(localStorage.getItem('cornell-diary:theme')).toBe('dark');
  });

  it('updates language', () => {
    useSettingsStore.getState().setLanguage('en');
    expect(useSettingsStore.getState().language).toBe('en');
  });
});

describe('syncStore', () => {
  it('opens and closes dialogs', () => {
    useSyncStore.getState().openDialog('export');
    expect(useSyncStore.getState().dialog).toBe('export');
    useSyncStore.getState().closeDialog();
    expect(useSyncStore.getState().dialog).toBe('none');
  });

  it('tracks last result', () => {
    const r = { inserted: 1, updated: 0, skipped: 0, errors: [] };
    useSyncStore.getState().setLastResult(r);
    expect(useSyncStore.getState().lastResult).toEqual(r);
  });
});
