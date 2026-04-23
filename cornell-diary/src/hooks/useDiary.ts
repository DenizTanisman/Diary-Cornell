import { useCallback, useEffect, useRef, useState } from 'react';
import type { CueItem, DiaryEntry } from '../types/diary';
import { createEmptyEntry, MAX_CUE_ITEMS } from '../types/diary';
import { useRepository } from '../db/RepositoryContext';
import { getDeviceId } from '../utils/deviceId';
import { AUTO_SAVE_DEBOUNCE_MS } from '../constants/config';
import { sanitizeText, sanitizeTitle } from '../utils/sanitize';
import { logger } from '../utils/logger';

interface UseDiaryOptions {
  date: string;
  autoSaveMs?: number;
}

export interface UseDiaryReturn {
  entry: DiaryEntry | null;
  isLoading: boolean;
  isSaving: boolean;
  isDirty: boolean;
  error: Error | null;

  updateDiary: (text: string) => void;
  updateSummary: (text: string) => void;
  updateQuote: (text: string) => void;
  addCueItem: (title: string) => void;
  updateCueItem: (position: number, changes: Partial<Omit<CueItem, 'position'>>) => void;
  removeCueItem: (position: number) => void;

  saveNow: () => Promise<void>;
  reload: () => Promise<void>;
}

export function useDiary({
  date,
  autoSaveMs = AUTO_SAVE_DEBOUNCE_MS,
}: UseDiaryOptions): UseDiaryReturn {
  const repository = useRepository();
  const [entry, setEntry] = useState<DiaryEntry | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isDirty, setIsDirty] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestEntryRef = useRef<DiaryEntry | null>(null);

  const load = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const existing = await repository.getByDate(date);
      const deviceId = await getDeviceId();
      const loaded = existing ?? createEmptyEntry(date, deviceId);
      setEntry(loaded);
      latestEntryRef.current = loaded;
      setIsDirty(false);
    } catch (err) {
      logger.error('diary_load_failed', { date, error: String(err) });
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setIsLoading(false);
    }
  }, [date, repository]);

  useEffect(() => {
    void load();
    return () => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    };
  }, [load]);

  const scheduleSave = useCallback(() => {
    setIsDirty(true);
    if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    saveTimeoutRef.current = setTimeout(async () => {
      const snapshot = latestEntryRef.current;
      if (!snapshot) return;
      setIsSaving(true);
      try {
        const saved = await repository.upsert(snapshot);
        latestEntryRef.current = saved;
        setEntry(saved);
        setIsDirty(false);
      } catch (err) {
        logger.error('diary_save_failed', { date: snapshot.date, error: String(err) });
        setError(err instanceof Error ? err : new Error(String(err)));
      } finally {
        setIsSaving(false);
      }
    }, autoSaveMs);
  }, [autoSaveMs, repository]);

  const mutate = useCallback(
    (updater: (prev: DiaryEntry) => DiaryEntry) => {
      setEntry((prev) => {
        if (!prev) return prev;
        const next = updater(prev);
        latestEntryRef.current = next;
        return next;
      });
      scheduleSave();
    },
    [scheduleSave],
  );

  const updateDiary = useCallback(
    (text: string) => mutate((prev) => ({ ...prev, diary: sanitizeText(text) })),
    [mutate],
  );

  const updateSummary = useCallback(
    (text: string) => mutate((prev) => ({ ...prev, summary: sanitizeText(text, 2000) })),
    [mutate],
  );

  const updateQuote = useCallback(
    (text: string) => mutate((prev) => ({ ...prev, quote: sanitizeText(text, 2000) })),
    [mutate],
  );

  const addCueItem = useCallback(
    (title: string) => {
      mutate((prev) => {
        if (prev.cueItems.length >= MAX_CUE_ITEMS) return prev;
        const used = new Set(prev.cueItems.map((c) => c.position));
        let position = 1;
        while (used.has(position) && position <= MAX_CUE_ITEMS) position++;
        const newItem: CueItem = {
          position,
          title: sanitizeTitle(title),
          content: '',
        };
        return { ...prev, cueItems: [...prev.cueItems, newItem] };
      });
    },
    [mutate],
  );

  const updateCueItem = useCallback(
    (position: number, changes: Partial<Omit<CueItem, 'position'>>) => {
      mutate((prev) => ({
        ...prev,
        cueItems: prev.cueItems.map((c) => {
          if (c.position !== position) return c;
          return {
            ...c,
            title: changes.title !== undefined ? sanitizeTitle(changes.title) : c.title,
            content: changes.content !== undefined ? sanitizeText(changes.content) : c.content,
          };
        }),
      }));
    },
    [mutate],
  );

  const removeCueItem = useCallback(
    (position: number) => {
      mutate((prev) => ({
        ...prev,
        cueItems: prev.cueItems.filter((c) => c.position !== position),
      }));
    },
    [mutate],
  );

  const saveNow = useCallback(async () => {
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }
    const snapshot = latestEntryRef.current;
    if (!snapshot) return;
    setIsSaving(true);
    try {
      const saved = await repository.upsert(snapshot);
      latestEntryRef.current = saved;
      setEntry(saved);
      setIsDirty(false);
    } finally {
      setIsSaving(false);
    }
  }, [repository]);

  return {
    entry,
    isLoading,
    isSaving,
    isDirty,
    error,
    updateDiary,
    updateSummary,
    updateQuote,
    addCueItem,
    updateCueItem,
    removeCueItem,
    saveNow,
    reload: load,
  };
}
