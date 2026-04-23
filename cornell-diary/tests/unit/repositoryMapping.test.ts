import { describe, expect, it } from 'vitest';
import { entryToRow, rowToEntry } from '../../src/db/SQLiteRepository';
import type { DiaryEntry } from '../../src/types/diary';

function make(): DiaryEntry {
  return {
    date: '2026-04-23',
    diary: 'main',
    cueItems: [
      { position: 1, title: 'Planlar', content: 'a' },
      { position: 3, title: 'Hissiyat', content: 'b' },
    ],
    summary: 's',
    quote: 'q',
    createdAt: '2026-04-23T09:00:00.000Z',
    updatedAt: '2026-04-23T12:00:00.000Z',
    deviceId: 'dev-1',
    version: 2,
  };
}

describe('row <-> entry mapping', () => {
  it('round-trips without losing data', () => {
    const original = make();
    const row = entryToRow(original);
    expect(row.title_1).toBe('Planlar');
    expect(row.title_2).toBeNull();
    expect(row.title_3).toBe('Hissiyat');
    const round = rowToEntry(row);
    expect(round).toEqual(original);
  });

  it('sorts cue items by position after reading from row', () => {
    const entry = make();
    entry.cueItems = [
      { position: 7, title: 'Z', content: 'z' },
      { position: 2, title: 'B', content: 'b' },
    ];
    const row = entryToRow(entry);
    const back = rowToEntry(row);
    expect(back.cueItems.map((c) => c.position)).toEqual([2, 7]);
  });
});
