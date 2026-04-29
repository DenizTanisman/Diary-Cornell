/**
 * Verifies TauriRepository invokes the right Rust command with the
 * exact argument shape Rust expects (camelCase keys via serde rename).
 *
 * We mock `@tauri-apps/api/core` so no real Tauri runtime is needed —
 * the assertions run as pure JS under vitest.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { TauriRepository } from '../../src/db/TauriRepository';
import type { DiaryEntry } from '../../src/types/diary';

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function sampleEntry(date = '2026-04-29'): DiaryEntry {
  return {
    date,
    diary: 'ben uğurböceğinden korkarım',
    cueItems: [
      { position: 1, title: 'Reflection', content: 'deep' },
      { position: 3, title: 'Goal', content: 'ship phase 1' },
    ],
    summary: 'good day',
    quote: 'carpe diem',
    createdAt: '2026-04-29T10:00:00.000Z',
    updatedAt: '2026-04-29T10:00:00.000Z',
    deviceId: 'test-device',
    version: 1,
  };
}

describe('TauriRepository', () => {
  let repo: TauriRepository;

  beforeEach(() => {
    invokeMock.mockReset();
    repo = new TauriRepository();
  });

  afterEach(() => {
    invokeMock.mockReset();
  });

  it('init is a no-op (Rust setup hook bootstraps the repo)', async () => {
    await expect(repo.init()).resolves.toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('getByDate forwards the date to diary_get_by_date', async () => {
    const entry = sampleEntry();
    invokeMock.mockResolvedValueOnce(entry);
    const got = await repo.getByDate('2026-04-29');
    expect(invokeMock).toHaveBeenCalledWith('diary_get_by_date', { date: '2026-04-29' });
    expect(got).toEqual(entry);
  });

  it('getByDate returns null when Rust says null', async () => {
    invokeMock.mockResolvedValueOnce(null);
    const got = await repo.getByDate('2030-01-01');
    expect(got).toBeNull();
  });

  it('upsert wraps the entry under the `entry` key', async () => {
    const entry = sampleEntry();
    invokeMock.mockResolvedValueOnce(entry);
    const saved = await repo.upsert(entry);
    expect(invokeMock).toHaveBeenCalledWith('diary_upsert', { entry });
    expect(saved).toEqual(entry);
  });

  it('delete passes only the date', async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await repo.delete('2026-04-29');
    expect(invokeMock).toHaveBeenCalledWith('diary_delete', { date: '2026-04-29' });
  });

  it('getRange uses camelCase startDate/endDate to match the Rust handler signature', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await repo.getRange('2026-04-01', '2026-04-30');
    expect(invokeMock).toHaveBeenCalledWith('diary_list_range', {
      startDate: '2026-04-01',
      endDate: '2026-04-30',
    });
  });

  it('search defaults limit to 50', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await repo.search('uğurböceğ');
    expect(invokeMock).toHaveBeenCalledWith('diary_search', { query: 'uğurböceğ', limit: 50 });
  });

  it('search forwards a custom limit', async () => {
    invokeMock.mockResolvedValueOnce([]);
    await repo.search('q', 10);
    expect(invokeMock).toHaveBeenCalledWith('diary_search', { query: 'q', limit: 10 });
  });

  it('getEntryCount coerces the Rust i64 result to number', async () => {
    invokeMock.mockResolvedValueOnce(7);
    const n = await repo.getEntryCount();
    expect(n).toBe(7);
  });

  it('bulkUpsert forwards entries verbatim', async () => {
    const entries = [sampleEntry('2026-04-28'), sampleEntry('2026-04-29')];
    invokeMock.mockResolvedValueOnce({ inserted: 2, updated: 0, skipped: 0 });
    const result = await repo.bulkUpsert(entries);
    expect(invokeMock).toHaveBeenCalledWith('diary_bulk_upsert', { entries });
    expect(result).toEqual({ inserted: 2, updated: 0, skipped: 0 });
  });

  it('settings round-trip through diary_get_setting / diary_set_setting', async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await repo.setSetting('theme', 'dark');
    expect(invokeMock).toHaveBeenCalledWith('diary_set_setting', { key: 'theme', value: 'dark' });

    invokeMock.mockResolvedValueOnce('dark');
    const v = await repo.getSetting('theme');
    expect(invokeMock).toHaveBeenLastCalledWith('diary_get_setting', { key: 'theme' });
    expect(v).toBe('dark');
  });
});
