import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ReactNode } from 'react';
import { useDiary } from '../../src/hooks/useDiary';
import { RepositoryProvider } from '../../src/db/RepositoryContext';
import type { BulkResult, IDiaryRepository } from '../../src/db/IDiaryRepository';
import type { DiaryEntry } from '../../src/types/diary';

class MemRepo implements IDiaryRepository {
  store = new Map<string, DiaryEntry>();
  upsertCalls = 0;
  async init() {}
  async getByDate(date: string) {
    return this.store.get(date) ?? null;
  }
  async upsert(entry: DiaryEntry) {
    this.upsertCalls++;
    const saved = { ...entry, version: entry.version + 1 };
    this.store.set(entry.date, saved);
    return saved;
  }
  async delete(date: string) {
    this.store.delete(date);
  }
  async getAllDates() {
    return [...this.store.keys()];
  }
  async getRange() {
    return [];
  }
  async getAll() {
    return [];
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
  async bulkUpsert(): Promise<BulkResult> {
    return { inserted: 0, updated: 0, skipped: 0 };
  }
  async getSetting() {
    return null;
  }
  async setSetting() {}
}

function makeWrapper(repo: IDiaryRepository) {
  return ({ children }: { children: ReactNode }) => (
    <RepositoryProvider repository={repo}>{children}</RepositoryProvider>
  );
}

describe('useDiary', () => {
  it('loads an empty entry when none exists', async () => {
    const repo = new MemRepo();
    const { result } = renderHook(() => useDiary({ date: '2026-04-23', autoSaveMs: 20 }), {
      wrapper: makeWrapper(repo),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.entry?.date).toBe('2026-04-23');
    expect(result.current.entry?.diary).toBe('');
  });

  it('debounces and persists diary text', async () => {
    vi.useFakeTimers();
    const repo = new MemRepo();
    const { result } = renderHook(() => useDiary({ date: '2026-04-23', autoSaveMs: 100 }), {
      wrapper: makeWrapper(repo),
    });

    await vi.waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.updateDiary('a'));
    act(() => result.current.updateDiary('ab'));
    act(() => result.current.updateDiary('abc'));
    expect(result.current.isDirty).toBe(true);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(150);
    });
    vi.useRealTimers();

    await waitFor(() => expect(repo.upsertCalls).toBe(1));
    expect(repo.store.get('2026-04-23')?.diary).toBe('abc');
  });

  it('adds, updates and removes cue items', async () => {
    const repo = new MemRepo();
    const { result } = renderHook(() => useDiary({ date: '2026-04-23', autoSaveMs: 50 }), {
      wrapper: makeWrapper(repo),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    act(() => result.current.addCueItem('Planlar'));
    expect(result.current.entry?.cueItems).toHaveLength(1);
    expect(result.current.entry?.cueItems[0].position).toBe(1);

    act(() => result.current.updateCueItem(1, { content: 'yap' }));
    expect(result.current.entry?.cueItems[0].content).toBe('yap');

    act(() => result.current.removeCueItem(1));
    expect(result.current.entry?.cueItems).toHaveLength(0);
  });

  it('enforces max 7 cue items', async () => {
    const repo = new MemRepo();
    const { result } = renderHook(() => useDiary({ date: '2026-04-23', autoSaveMs: 50 }), {
      wrapper: makeWrapper(repo),
    });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    for (let i = 0; i < 9; i++) {
      act(() => result.current.addCueItem(`T${i}`));
    }
    expect(result.current.entry?.cueItems).toHaveLength(7);
  });
});
