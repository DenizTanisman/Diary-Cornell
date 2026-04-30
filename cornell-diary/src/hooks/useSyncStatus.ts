import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { SyncStatus } from '../types/cloudSync';

/** Polling interval for the sync status chip. 5 s matches the prompt's spec
 *  and is comfortable for a desktop app — not a Cloud round-trip, just a
 *  local DB read.
 */
const POLL_MS = 5000;

/**
 * Polls `get_sync_status` every 5 seconds. Returns the latest snapshot plus
 * a `refresh()` for callers (e.g. SyncSettings after a manual trigger) that
 * want an immediate update without waiting for the next tick.
 */
export function useSyncStatus() {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let cancelled = false;
    const fetchOnce = async () => {
      try {
        const next = await invoke<SyncStatus>('get_sync_status');
        if (!cancelled) {
          setStatus(next);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          // Don't clear the previous status — UI keeps its last known state
          // and just surfaces the error string for diagnosis.
          setError(extractMessage(e));
        }
      }
    };
    void fetchOnce();
    const id = window.setInterval(() => void fetchOnce(), POLL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [tick]);

  return {
    status,
    error,
    refresh: () => setTick((n) => n + 1),
  };
}

function extractMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object') {
    const env = e as { code?: string; message?: string };
    if (env.message) return env.code ? `[${env.code}] ${env.message}` : env.message;
  }
  return 'unknown error';
}
