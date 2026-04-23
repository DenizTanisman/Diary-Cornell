import { describe, expect, it } from 'vitest';
import { resolveConflict } from '../../src/sync/conflictResolver';
import type { DiaryEntry } from '../../src/types/diary';

function make(overrides: Partial<DiaryEntry>): DiaryEntry {
  return {
    date: '2026-04-23',
    diary: '',
    cueItems: [],
    summary: '',
    quote: '',
    createdAt: '2026-04-23T00:00:00.000Z',
    updatedAt: '2026-04-23T10:00:00.000Z',
    version: 1,
    ...overrides,
  };
}

describe('resolveConflict', () => {
  it('prefers remote when remote is newer', () => {
    const local = make({ updatedAt: '2026-04-22T10:00:00Z' });
    const remote = make({ updatedAt: '2026-04-23T10:00:00Z' });
    const { winner, reason } = resolveConflict(local, remote);
    expect(winner).toBe(remote);
    expect(reason).toBe('remote_newer');
  });

  it('prefers local when local is newer', () => {
    const local = make({ updatedAt: '2026-04-24T10:00:00Z' });
    const remote = make({ updatedAt: '2026-04-23T10:00:00Z' });
    const { winner, reason } = resolveConflict(local, remote);
    expect(winner).toBe(local);
    expect(reason).toBe('local_newer');
  });

  it('uses version on timestamp tie', () => {
    const local = make({ version: 1 });
    const remote = make({ version: 3 });
    const { winner, reason } = resolveConflict(local, remote);
    expect(winner).toBe(remote);
    expect(reason).toBe('remote_higher_version');
  });

  it('deterministically prefers local on full tie', () => {
    const local = make({ version: 2 });
    const remote = make({ version: 2 });
    const { winner, reason } = resolveConflict(local, remote);
    expect(winner).toBe(local);
    expect(reason).toBe('tie_local_preferred');
  });
});
