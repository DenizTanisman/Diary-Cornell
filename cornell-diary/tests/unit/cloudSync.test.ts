/**
 * Pure-logic tests for the Cloud sync indicator state machine. The IPC
 * itself (CloudSyncPanel, useSyncStatus) is exercised by the existing
 * integration test runner; here we only assert the state derivation has
 * no off-by-ones.
 */
import { describe, expect, it } from 'vitest';

import { deriveIndicatorState, type SyncStatus } from '../../src/types/cloudSync';

function status(partial: Partial<SyncStatus>): SyncStatus {
  return {
    enabled: false,
    online: false,
    lastPullAt: null,
    lastPushAt: null,
    dirtyCount: 0,
    ...partial,
  };
}

describe('deriveIndicatorState', () => {
  it('treats a missing snapshot as disabled', () => {
    expect(deriveIndicatorState(null)).toBe('disabled');
  });

  it('treats a snapshot with enabled=false as disabled', () => {
    expect(deriveIndicatorState(status({ enabled: false, online: true }))).toBe('disabled');
  });

  it('treats enabled but offline as offline', () => {
    expect(deriveIndicatorState(status({ enabled: true, online: false }))).toBe('offline');
  });

  it('treats online + dirty count > 0 as dirty', () => {
    expect(
      deriveIndicatorState(status({ enabled: true, online: true, dirtyCount: 3 })),
    ).toBe('dirty');
  });

  it('treats online + zero dirty as synced', () => {
    expect(
      deriveIndicatorState(status({ enabled: true, online: true, dirtyCount: 0 })),
    ).toBe('synced');
  });
});
