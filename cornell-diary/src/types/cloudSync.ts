/**
 * Wire shapes for the Cloud sync IPC. Mirrors the Rust DTOs in
 * `src-tauri/src/sync/models.rs` (camelCase via #[serde(rename_all)]) and
 * `src-tauri/src/commands/sync.rs`. Every field here is what `invoke()`
 * actually sees coming back from Rust.
 */
export interface ConnectReport {
  userId: string | null;
  peerId: string;
  journalId: string;
  journalName: string;
}

export interface SyncStatus {
  enabled: boolean;
  online: boolean;
  lastPullAt: string | null;
  lastPushAt: string | null;
  dirtyCount: number;
}

export interface SyncReport {
  pulled: number;
  pushed: number;
  conflictsCloudWon: number;
  conflictsLocalWon: number;
  rejected: number;
  durationMs: number;
}

export type SyncIndicatorState = 'disabled' | 'offline' | 'dirty' | 'synced';

export function deriveIndicatorState(s: SyncStatus | null): SyncIndicatorState {
  if (!s || !s.enabled) return 'disabled';
  if (!s.online) return 'offline';
  if (s.dirtyCount > 0) return 'dirty';
  return 'synced';
}
