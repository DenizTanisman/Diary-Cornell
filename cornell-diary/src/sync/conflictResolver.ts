import type { DiaryEntry } from '../types/diary';

export type ConflictReason =
  | 'remote_newer'
  | 'local_newer'
  | 'remote_higher_version'
  | 'tie_local_preferred';

export interface ConflictResolution {
  winner: DiaryEntry;
  reason: ConflictReason;
}

export function resolveConflict(
  local: DiaryEntry,
  remote: DiaryEntry,
): ConflictResolution {
  const localTime = new Date(local.updatedAt).getTime();
  const remoteTime = new Date(remote.updatedAt).getTime();

  if (remoteTime > localTime) return { winner: remote, reason: 'remote_newer' };
  if (localTime > remoteTime) return { winner: local, reason: 'local_newer' };

  if (remote.version > local.version) return { winner: remote, reason: 'remote_higher_version' };

  return { winner: local, reason: 'tie_local_preferred' };
}
