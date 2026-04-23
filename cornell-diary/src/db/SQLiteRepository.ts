import Database from '@tauri-apps/plugin-sql';
import type { BulkResult, IDiaryRepository } from './IDiaryRepository';
import type { CueItem, DbDiaryRow, DiaryEntry } from '../types/diary';
import { MAX_CUE_ITEMS } from '../types/diary';
import { DB_NAME } from '../constants/config';
import { isValidISODate } from '../utils/date';

export class SQLiteRepository implements IDiaryRepository {
  private db: Database | null = null;

  async init(): Promise<void> {
    if (this.db) return;
    this.db = await Database.load(DB_NAME);
    try {
      await this.db.execute('PRAGMA journal_mode = WAL');
      await this.db.execute('PRAGMA foreign_keys = ON');
    } catch {
      // pragmas are best-effort
    }
  }

  private ensureDb(): Database {
    if (!this.db) throw new Error('SQLiteRepository not initialized. Call init() first.');
    return this.db;
  }

  async getByDate(date: string): Promise<DiaryEntry | null> {
    this.validateDate(date);
    const rows = await this.ensureDb().select<DbDiaryRow[]>(
      'SELECT * FROM diary_entries WHERE date = $1',
      [date],
    );
    return rows.length > 0 ? rowToEntry(rows[0]) : null;
  }

  async upsert(entry: DiaryEntry): Promise<DiaryEntry> {
    this.validateDate(entry.date);
    this.validateCueItems(entry.cueItems);

    const now = new Date().toISOString();
    const row = entryToRow({ ...entry, updatedAt: now });

    await this.ensureDb().execute(
      `INSERT INTO diary_entries (
        date, diary,
        title_1, content_1, title_2, content_2, title_3, content_3,
        title_4, content_4, title_5, content_5, title_6, content_6,
        title_7, content_7,
        summary, quote, created_at, updated_at, device_id, version
      ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
        $15, $16, $17, $18, $19, $20, $21, $22
      )
      ON CONFLICT(date) DO UPDATE SET
        diary = excluded.diary,
        title_1 = excluded.title_1, content_1 = excluded.content_1,
        title_2 = excluded.title_2, content_2 = excluded.content_2,
        title_3 = excluded.title_3, content_3 = excluded.content_3,
        title_4 = excluded.title_4, content_4 = excluded.content_4,
        title_5 = excluded.title_5, content_5 = excluded.content_5,
        title_6 = excluded.title_6, content_6 = excluded.content_6,
        title_7 = excluded.title_7, content_7 = excluded.content_7,
        summary = excluded.summary,
        quote = excluded.quote,
        updated_at = excluded.updated_at,
        device_id = excluded.device_id,
        version = diary_entries.version + 1`,
      [
        row.date,
        row.diary,
        row.title_1,
        row.content_1,
        row.title_2,
        row.content_2,
        row.title_3,
        row.content_3,
        row.title_4,
        row.content_4,
        row.title_5,
        row.content_5,
        row.title_6,
        row.content_6,
        row.title_7,
        row.content_7,
        row.summary,
        row.quote,
        row.created_at,
        row.updated_at,
        row.device_id,
        row.version,
      ],
    );

    const saved = await this.getByDate(entry.date);
    if (!saved) throw new Error(`Upsert succeeded but read failed for ${entry.date}`);
    return saved;
  }

  async delete(date: string): Promise<void> {
    this.validateDate(date);
    await this.ensureDb().execute('DELETE FROM diary_entries WHERE date = $1', [date]);
  }

  async getAllDates(): Promise<string[]> {
    const rows = await this.ensureDb().select<{ date: string }[]>(
      'SELECT date FROM diary_entries ORDER BY date DESC',
    );
    return rows.map((r) => r.date);
  }

  async getRange(startDate: string, endDate: string): Promise<DiaryEntry[]> {
    this.validateDate(startDate);
    this.validateDate(endDate);
    const rows = await this.ensureDb().select<DbDiaryRow[]>(
      'SELECT * FROM diary_entries WHERE date >= $1 AND date <= $2 ORDER BY date DESC',
      [startDate, endDate],
    );
    return rows.map(rowToEntry);
  }

  async getAll(): Promise<DiaryEntry[]> {
    const rows = await this.ensureDb().select<DbDiaryRow[]>(
      'SELECT * FROM diary_entries ORDER BY date DESC',
    );
    return rows.map(rowToEntry);
  }

  async search(query: string, limit = 50): Promise<DiaryEntry[]> {
    const q = `%${query}%`;
    const rows = await this.ensureDb().select<DbDiaryRow[]>(
      `SELECT * FROM diary_entries
       WHERE diary LIKE $1 OR summary LIKE $1 OR quote LIKE $1
          OR content_1 LIKE $1 OR content_2 LIKE $1 OR content_3 LIKE $1
          OR content_4 LIKE $1 OR content_5 LIKE $1 OR content_6 LIKE $1
          OR content_7 LIKE $1
          OR title_1 LIKE $1 OR title_2 LIKE $1 OR title_3 LIKE $1
          OR title_4 LIKE $1 OR title_5 LIKE $1 OR title_6 LIKE $1
          OR title_7 LIKE $1
       ORDER BY date DESC LIMIT $2`,
      [q, limit],
    );
    return rows.map(rowToEntry);
  }

  async getEntryCount(): Promise<number> {
    const rows = await this.ensureDb().select<{ count: number }[]>(
      'SELECT COUNT(*) as count FROM diary_entries',
    );
    return rows[0]?.count ?? 0;
  }

  async getLastUpdatedAt(): Promise<string | null> {
    const rows = await this.ensureDb().select<{ updated_at: string }[]>(
      'SELECT updated_at FROM diary_entries ORDER BY updated_at DESC LIMIT 1',
    );
    return rows[0]?.updated_at ?? null;
  }

  async bulkUpsert(entries: DiaryEntry[]): Promise<BulkResult> {
    // Protective merge: existing dates are NEVER overwritten from a foreign export,
    // even when the remote copy is newer. Only brand-new dates are inserted.
    // NOTE: no outer BEGIN/COMMIT — tauri-plugin-sql's pool can hand out a different
    // connection per call, which leaves the transaction orphaned. Each upsertRaw is atomic.
    let inserted = 0;
    const updated = 0;
    let skipped = 0;

    for (const entry of entries) {
      const existing = await this.getByDate(entry.date);
      if (!existing) {
        await this.upsertRaw(entry);
        inserted++;
      } else {
        skipped++;
      }
    }

    return { inserted, updated, skipped };
  }

  private async upsertRaw(entry: DiaryEntry): Promise<void> {
    this.validateDate(entry.date);
    this.validateCueItems(entry.cueItems);
    const row = entryToRow(entry);
    await this.ensureDb().execute(
      `INSERT INTO diary_entries (
        date, diary,
        title_1, content_1, title_2, content_2, title_3, content_3,
        title_4, content_4, title_5, content_5, title_6, content_6,
        title_7, content_7,
        summary, quote, created_at, updated_at, device_id, version
      ) VALUES (
        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
        $15, $16, $17, $18, $19, $20, $21, $22
      )
      ON CONFLICT(date) DO UPDATE SET
        diary = excluded.diary,
        title_1 = excluded.title_1, content_1 = excluded.content_1,
        title_2 = excluded.title_2, content_2 = excluded.content_2,
        title_3 = excluded.title_3, content_3 = excluded.content_3,
        title_4 = excluded.title_4, content_4 = excluded.content_4,
        title_5 = excluded.title_5, content_5 = excluded.content_5,
        title_6 = excluded.title_6, content_6 = excluded.content_6,
        title_7 = excluded.title_7, content_7 = excluded.content_7,
        summary = excluded.summary,
        quote = excluded.quote,
        updated_at = excluded.updated_at,
        device_id = excluded.device_id,
        version = excluded.version`,
      [
        row.date,
        row.diary,
        row.title_1,
        row.content_1,
        row.title_2,
        row.content_2,
        row.title_3,
        row.content_3,
        row.title_4,
        row.content_4,
        row.title_5,
        row.content_5,
        row.title_6,
        row.content_6,
        row.title_7,
        row.content_7,
        row.summary,
        row.quote,
        row.created_at,
        row.updated_at,
        row.device_id,
        row.version,
      ],
    );
  }

  async getSetting(key: string): Promise<string | null> {
    const rows = await this.ensureDb().select<{ value: string }[]>(
      'SELECT value FROM app_settings WHERE key = $1',
      [key],
    );
    return rows[0]?.value ?? null;
  }

  async setSetting(key: string, value: string): Promise<void> {
    const now = new Date().toISOString();
    await this.ensureDb().execute(
      `INSERT INTO app_settings (key, value, updated_at)
       VALUES ($1, $2, $3)
       ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at`,
      [key, value, now],
    );
  }

  private validateDate(date: string): void {
    if (!isValidISODate(date)) {
      throw new Error(`Invalid date: ${date} (expected YYYY-MM-DD)`);
    }
  }

  private validateCueItems(items: CueItem[]): void {
    if (items.length > MAX_CUE_ITEMS) {
      throw new Error(`Too many cue items: ${items.length} (max ${MAX_CUE_ITEMS})`);
    }
    const positions = new Set<number>();
    for (const item of items) {
      if (item.position < 1 || item.position > MAX_CUE_ITEMS) {
        throw new Error(`Invalid position: ${item.position}`);
      }
      if (positions.has(item.position)) {
        throw new Error(`Duplicate position: ${item.position}`);
      }
      positions.add(item.position);
    }
  }
}

export function rowToEntry(row: DbDiaryRow): DiaryEntry {
  const cueItems: CueItem[] = [];
  for (let i = 1; i <= MAX_CUE_ITEMS; i++) {
    const title = row[`title_${i}` as keyof DbDiaryRow] as string | null;
    const content = row[`content_${i}` as keyof DbDiaryRow] as string | null;
    if (title !== null) {
      cueItems.push({ position: i, title, content: content ?? '' });
    }
  }
  cueItems.sort((a, b) => a.position - b.position);
  return {
    date: row.date,
    diary: row.diary,
    cueItems,
    summary: row.summary ?? '',
    quote: row.quote ?? '',
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    deviceId: row.device_id ?? undefined,
    version: row.version,
  };
}

export function entryToRow(entry: DiaryEntry): DbDiaryRow {
  const row: DbDiaryRow = {
    date: entry.date,
    diary: entry.diary,
    title_1: null,
    content_1: null,
    title_2: null,
    content_2: null,
    title_3: null,
    content_3: null,
    title_4: null,
    content_4: null,
    title_5: null,
    content_5: null,
    title_6: null,
    content_6: null,
    title_7: null,
    content_7: null,
    summary: entry.summary,
    quote: entry.quote,
    created_at: entry.createdAt,
    updated_at: entry.updatedAt,
    device_id: entry.deviceId ?? null,
    version: entry.version,
  };

  for (const item of entry.cueItems) {
    (row as unknown as Record<string, string | null>)[`title_${item.position}`] = item.title;
    (row as unknown as Record<string, string | null>)[`content_${item.position}`] = item.content;
  }

  return row;
}
