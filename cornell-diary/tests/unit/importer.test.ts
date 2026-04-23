import { describe, expect, it } from 'vitest';
import { exportToJSON, serializeExport } from '../../src/sync/exporter';
import { ImportError, importFromJSON, parseExportFile } from '../../src/sync/importer';
import type { DiaryEntry } from '../../src/types/diary';
import type { IDiaryRepository, BulkResult } from '../../src/db/IDiaryRepository';

class MemRepo implements IDiaryRepository {
  private store = new Map<string, DiaryEntry>();
  async init() {}
  async getByDate(date: string) {
    return this.store.get(date) ?? null;
  }
  async upsert(entry: DiaryEntry) {
    this.store.set(entry.date, entry);
    return entry;
  }
  async delete(date: string) {
    this.store.delete(date);
  }
  async getAllDates() {
    return [...this.store.keys()].sort();
  }
  async getRange(a: string, b: string) {
    return [...this.store.values()].filter((e) => e.date >= a && e.date <= b);
  }
  async getAll() {
    return [...this.store.values()];
  }
  async search() {
    return [];
  }
  async getEntryCount() {
    return this.store.size;
  }
  async getLastUpdatedAt() {
    return null;
  }
  async bulkUpsert(entries: DiaryEntry[]): Promise<BulkResult> {
    // Mirrors SQLiteRepository: protective skip-on-conflict semantics.
    let inserted = 0;
    let skipped = 0;
    for (const e of entries) {
      if (!this.store.has(e.date)) {
        this.store.set(e.date, e);
        inserted++;
      } else {
        skipped++;
      }
    }
    return { inserted, updated: 0, skipped };
  }
  async getSetting() {
    return null;
  }
  async setSetting() {}
}

function entry(date: string, diary: string): DiaryEntry {
  return {
    date,
    diary,
    cueItems: [{ position: 1, title: 'Planlar', content: 'a' }],
    summary: 'ok',
    quote: 'q',
    createdAt: `${date}T00:00:00.000Z`,
    updatedAt: `${date}T12:00:00.000Z`,
    version: 1,
  };
}

describe('importer', () => {
  it('rejects invalid JSON', async () => {
    await expect(parseExportFile('{not json')).rejects.toBeInstanceOf(ImportError);
  });

  it('rejects schema-invalid payload', async () => {
    await expect(parseExportFile('{}')).rejects.toBeInstanceOf(ImportError);
  });

  it('imports valid export into repository', async () => {
    const entries = [entry('2026-04-23', 'a'), entry('2026-04-22', 'b')];
    const file = await exportToJSON(entries, 'dev');
    const raw = await serializeExport(file);
    const repo = new MemRepo();
    const result = await importFromJSON(raw, repo);
    expect(result.inserted).toBe(2);
    expect(result.updated).toBe(0);
    expect(result.skipped).toBe(0);
    expect(await repo.getEntryCount()).toBe(2);
  });

  it('rejects on checksum mismatch unless overridden', async () => {
    const entries = [entry('2026-04-23', 'a')];
    const file = await exportToJSON(entries, 'dev');
    file.checksum = 'sha256:' + 'f'.repeat(64);
    const raw = JSON.stringify(file);
    const repo = new MemRepo();
    await expect(importFromJSON(raw, repo)).rejects.toMatchObject({
      code: 'checksum_mismatch',
    });
    const result = await importFromJSON(raw, repo, { ignoreChecksumMismatch: true });
    expect(result.inserted).toBe(1);
  });

  it('skips older remote entries on second import', async () => {
    const entries = [entry('2026-04-23', 'first')];
    const file = await exportToJSON(entries, 'dev');
    const raw = await serializeExport(file);
    const repo = new MemRepo();
    await importFromJSON(raw, repo);

    // local already has fresher copy
    await repo.upsert({
      ...entries[0],
      diary: 'fresher',
      updatedAt: '2030-01-01T00:00:00.000Z',
      version: 2,
    });

    const result = await importFromJSON(raw, repo);
    expect(result.skipped).toBe(1);
    const current = await repo.getByDate('2026-04-23');
    expect(current?.diary).toBe('fresher');
  });

  it('protects local entries even when remote is newer (skip-on-conflict)', async () => {
    const repo = new MemRepo();
    // local has older version
    await repo.upsert(entry('2026-04-23', 'local-original'));

    // remote is strictly newer — should still be skipped, local preserved
    const remote = [
      {
        ...entry('2026-04-23', 'remote-newer'),
        updatedAt: '2030-12-31T23:59:59.000Z',
        version: 99,
      },
    ];
    const file = await exportToJSON(remote, 'remote-device');
    const raw = await serializeExport(file);

    const result = await importFromJSON(raw, repo);
    expect(result.inserted).toBe(0);
    expect(result.updated).toBe(0);
    expect(result.skipped).toBe(1);

    const current = await repo.getByDate('2026-04-23');
    expect(current?.diary).toBe('local-original');
  });

  it('mixes: inserts new dates and skips colliding dates in one import', async () => {
    const repo = new MemRepo();
    await repo.upsert(entry('2026-04-20', 'local-keep'));

    const remote = [
      entry('2026-04-20', 'remote-conflict'),
      entry('2026-04-21', 'remote-new-a'),
      entry('2026-04-22', 'remote-new-b'),
    ];
    const file = await exportToJSON(remote, 'dev');
    const raw = await serializeExport(file);

    const result = await importFromJSON(raw, repo);
    expect(result.inserted).toBe(2);
    expect(result.skipped).toBe(1);
    expect((await repo.getByDate('2026-04-20'))?.diary).toBe('local-keep');
    expect((await repo.getByDate('2026-04-21'))?.diary).toBe('remote-new-a');
    expect((await repo.getByDate('2026-04-22'))?.diary).toBe('remote-new-b');
  });
});
