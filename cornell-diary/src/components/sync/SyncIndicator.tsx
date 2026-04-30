/**
 * Top-bar chip that shows where the local copy stands vs Cloud.
 *
 * - 🟢 synced     — connected, online, no dirty rows
 * - 🟡 dirty      — connected, online, dirty rows waiting to push
 * - 🔴 offline    — connected but Cloud /health is down (after 3 missed probes)
 * - ⚪ disabled   — not connected (no token in sync_metadata)
 */
import { Link } from 'react-router-dom';

import { useSyncStatus } from '../../hooks/useSyncStatus';
import { deriveIndicatorState, type SyncIndicatorState } from '../../types/cloudSync';

const LABEL: Record<SyncIndicatorState, string> = {
  disabled: 'Cloud bağlı değil',
  offline: 'Cloud erişilemiyor',
  dirty: 'Bekleyen değişiklik',
  synced: 'Senkron',
};

const COLOR: Record<SyncIndicatorState, string> = {
  disabled: '#8A8A8A',
  offline: '#BA2222',
  dirty: '#BA7517',
  synced: '#3B6D11',
};

const ICON: Record<SyncIndicatorState, string> = {
  disabled: '⚪',
  offline: '🔴',
  dirty: '🟡',
  synced: '🟢',
};

export function SyncIndicator() {
  const { status, error } = useSyncStatus();
  const state = deriveIndicatorState(status);
  const dirty = status?.dirtyCount ?? 0;
  const tooltip = error
    ? `Sync hatası: ${error}`
    : state === 'dirty'
      ? `${dirty} bekleyen değişiklik`
      : LABEL[state];

  return (
    <Link
      to="/sync"
      data-testid="sync-indicator"
      data-state={state}
      title={tooltip}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '0.35rem',
        padding: '0.25rem 0.6rem',
        borderRadius: '999px',
        border: `1px solid ${COLOR[state]}`,
        color: COLOR[state],
        fontSize: '0.78rem',
        textDecoration: 'none',
        background: 'transparent',
      }}
    >
      <span aria-hidden="true">{ICON[state]}</span>
      <span>{LABEL[state]}</span>
      {state === 'dirty' && (
        <span aria-hidden="true" style={{ opacity: 0.7 }}>
          ({dirty})
        </span>
      )}
    </Link>
  );
}
