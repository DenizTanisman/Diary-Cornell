import type { DiaryEntry } from '../types/diary';

export interface BulkResult {
  inserted: number;
  updated: number;
  skipped: number;
}

export interface IDiaryRepository {
  init(): Promise<void>;

  getByDate(date: string): Promise<DiaryEntry | null>;
  upsert(entry: DiaryEntry): Promise<DiaryEntry>;
  delete(date: string): Promise<void>;

  getAllDates(): Promise<string[]>;
  getRange(startDate: string, endDate: string): Promise<DiaryEntry[]>;
  getAll(): Promise<DiaryEntry[]>;

  search(query: string, limit?: number): Promise<DiaryEntry[]>;

  getEntryCount(): Promise<number>;
  getLastUpdatedAt(): Promise<string | null>;

  bulkUpsert(entries: DiaryEntry[]): Promise<BulkResult>;

  getSetting(key: string): Promise<string | null>;
  setSetting(key: string, value: string): Promise<void>;
}
