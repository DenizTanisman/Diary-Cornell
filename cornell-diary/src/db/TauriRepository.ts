import { invoke } from '@tauri-apps/api/core';

import type { BulkResult, IDiaryRepository } from './IDiaryRepository';
import type { DiaryEntry } from '../types/diary';

/**
 * IDiaryRepository implementation that defers to Rust via Tauri's IPC.
 *
 * Migration plan (diary_prompt_v2.md FAZ 1.0): every previous direct call
 * to `tauri-plugin-sql` is replaced with a typed `invoke('diary_*')`. The
 * underlying Rust handler is `SqliteEntryRepository` today; FAZ 1.1 swaps
 * it for `PostgresEntryRepository` with no change here.
 *
 * No init step is needed on the JS side — the Rust setup hook runs schema
 * migrations during application boot. We expose `init()` only to satisfy
 * the interface; it's a no-op.
 */
export class TauriRepository implements IDiaryRepository {
  async init(): Promise<void> {
    // Rust's `setup` hook in lib.rs already initialised the repository
    // before the WebView came up. Nothing to do here.
  }

  async getByDate(date: string): Promise<DiaryEntry | null> {
    return invoke<DiaryEntry | null>('diary_get_by_date', { date });
  }

  async upsert(entry: DiaryEntry): Promise<DiaryEntry> {
    return invoke<DiaryEntry>('diary_upsert', { entry });
  }

  async delete(date: string): Promise<void> {
    await invoke<void>('diary_delete', { date });
  }

  async getAllDates(): Promise<string[]> {
    return invoke<string[]>('diary_list_dates');
  }

  async getRange(startDate: string, endDate: string): Promise<DiaryEntry[]> {
    return invoke<DiaryEntry[]>('diary_list_range', { startDate, endDate });
  }

  async getAll(): Promise<DiaryEntry[]> {
    return invoke<DiaryEntry[]>('diary_list_all');
  }

  async search(query: string, limit = 50): Promise<DiaryEntry[]> {
    return invoke<DiaryEntry[]>('diary_search', { query, limit });
  }

  async getEntryCount(): Promise<number> {
    const n = await invoke<number>('diary_entry_count');
    return Number(n);
  }

  async getLastUpdatedAt(): Promise<string | null> {
    return invoke<string | null>('diary_last_updated_at');
  }

  async bulkUpsert(entries: DiaryEntry[]): Promise<BulkResult> {
    const result = await invoke<{ inserted: number; updated: number; skipped: number }>(
      'diary_bulk_upsert',
      { entries },
    );
    return result;
  }

  async getSetting(key: string): Promise<string | null> {
    return invoke<string | null>('diary_get_setting', { key });
  }

  async setSetting(key: string, value: string): Promise<void> {
    await invoke<void>('diary_set_setting', { key, value });
  }
}
