import { describe, expect, it } from 'vitest';
import { canonicalizeEntries, exportToJSON, serializeExport } from '../../src/sync/exporter';
import { sha256 } from '../../src/utils/crypto';
import type { DiaryEntry } from '../../src/types/diary';

function entry(date: string, diary: string): DiaryEntry {
  return {
    date,
    diary,
    cueItems: [],
    summary: '',
    quote: '',
    createdAt: `${date}T00:00:00.000Z`,
    updatedAt: `${date}T10:00:00.000Z`,
    version: 1,
  };
}

describe('exportToJSON', () => {
  it('produces a well-formed envelope with correct checksum', async () => {
    const entries = [entry('2026-04-23', 'hi'), entry('2026-04-20', 'older')];
    const file = await exportToJSON(entries, 'device-123');

    expect(file.format).toBe('cornell-diary-export');
    expect(file.version).toBe('1.0.0');
    expect(file.entryCount).toBe(2);
    expect(file.checksum).toMatch(/^sha256:[a-f0-9]{64}$/);
    expect(file.entries).toHaveLength(2);

    const expected = await sha256(canonicalizeEntries(entries));
    expect(file.checksum).toBe(expected);
  });

  it('round-trips through serialize', async () => {
    const entries = [entry('2026-04-23', 'hi')];
    const file = await exportToJSON(entries, 'device-abc');
    const str = await serializeExport(file);
    const parsed = JSON.parse(str);
    expect(parsed.checksum).toBe(file.checksum);
    expect(parsed.entries[0].date).toBe('2026-04-23');
  });
});
