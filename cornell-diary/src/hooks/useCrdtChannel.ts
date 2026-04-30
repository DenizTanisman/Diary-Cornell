/**
 * `useCrdtChannel(entryDate, fieldName, seedText)` — the React side of
 * FAZ 3 live editing.
 *
 * Lifecycle:
 *  - On mount: invoke `subscribe_crdt` so the Rust side opens the WS,
 *    seeds a `CrdtDocument` with whatever text the editor already has,
 *    and starts forwarding ops.
 *  - Listens to `crdt:text-updated` and applies remote text into local
 *    state. Listens to `crdt:presence` to flip `crdtMode` whenever a
 *    second peer joins.
 *  - On unmount: `unsubscribe_crdt`. The shared socket stays open so
 *    other entries keep working.
 *
 * `applyLocalText(newText)` is the call the editor makes on every
 * keystroke when `crdtMode` is true. Rust diffs against the live doc,
 * generates the CharOps, broadcasts them, and returns the new
 * materialised text. We feed that back into local state so the
 * textarea stays consistent with the doc.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { CrdtPresence, CrdtTextUpdated } from '../types/crdt';
import { logger } from '../utils/logger';

interface UseCrdtChannelArgs {
  /** ISO date the editor is currently looking at. */
  entryDate: string;
  /** Which column on that entry — `diary`, `summary`, `quote`, etc. */
  fieldName: string;
  /** Current saved text — used to seed the Rust CrdtDocument. */
  seedText: string;
  /** Toggled by the user's "live mode" preference. When false the
   *  channel sits inert (no subscribe, no listeners).             */
  enabled?: boolean;
}

export interface UseCrdtChannelReturn {
  /** Mirrored materialised text. Equals `seedText` until the first
   *  remote update or local typing lands.                        */
  text: string;
  /** Peer ids the server knows are subscribed to the same entry. */
  peers: string[];
  /** True iff `peers.length > 1`. The editor uses this to swap
   *  between debounced full-text save and per-keystroke ops.    */
  crdtMode: boolean;
  /** Last error from any `invoke` / `listen` call. UI surfaces it
   *  in a non-blocking way (the editor still works on the local
   *  Postgres save path even if the WS pipe is broken).        */
  error: string | null;
  /** Send a new full-text snapshot. Returns the text the doc
   *  ended up with after applying the diff (and any concurrent
   *  remote ops that arrived in the same tick).               */
  applyLocalText: (newText: string) => Promise<string>;
}

export function useCrdtChannel({
  entryDate,
  fieldName,
  seedText,
  enabled = true,
}: UseCrdtChannelArgs): UseCrdtChannelReturn {
  const [text, setText] = useState(seedText);
  const [peers, setPeers] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  // The seed only seeds *once* per (entryDate, fieldName) — switching
  // dates pulls a fresh seed in by re-running the subscribe effect.
  const seedRef = useRef(seedText);
  seedRef.current = seedText;

  useEffect(() => {
    if (!enabled) return;

    let cancelled = false;
    let unlistenText: UnlistenFn | undefined;
    let unlistenPresence: UnlistenFn | undefined;

    (async () => {
      try {
        unlistenText = await listen<CrdtTextUpdated>('crdt:text-updated', (event) => {
          const p = event.payload;
          if (p.entry_date === entryDate && p.field === fieldName) {
            setText(p.text);
          }
        });
        unlistenPresence = await listen<CrdtPresence>('crdt:presence', (event) => {
          if (cancelled) return;
          setPeers(event.payload.peers);
        });

        const initial = await invoke<string>('subscribe_crdt', {
          entryDate,
          fieldName,
          seedText: seedRef.current,
        });
        if (!cancelled) setText(initial);
      } catch (e) {
        if (!cancelled) setError(extractMessage(e));
        logger.warn('crdt_subscribe_failed', { entryDate, fieldName, error: String(e) });
      }
    })();

    return () => {
      cancelled = true;
      unlistenText?.();
      unlistenPresence?.();
      // Best-effort unsubscribe; ignore errors so unmount stays fast.
      void invoke('unsubscribe_crdt', { entryDate, fieldName }).catch(() => {});
    };
  }, [entryDate, fieldName, enabled]);

  const applyLocalText = useCallback(
    async (newText: string): Promise<string> => {
      try {
        const next = await invoke<string>('apply_local_text', {
          entryDate,
          fieldName,
          newText,
        });
        setText(next);
        return next;
      } catch (e) {
        setError(extractMessage(e));
        logger.warn('crdt_apply_local_failed', { entryDate, fieldName, error: String(e) });
        return newText;
      }
    },
    [entryDate, fieldName],
  );

  const crdtMode = peers.length > 1;

  return { text, peers, crdtMode, error, applyLocalText };
}

function extractMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object') {
    const env = e as { code?: string; message?: string };
    if (env.message) return env.code ? `[${env.code}] ${env.message}` : env.message;
  }
  return 'unknown error';
}
