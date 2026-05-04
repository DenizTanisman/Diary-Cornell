import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import type { CloudProfile } from '../types/cloudProfile';

/**
 * Subscribes to the active cloud profile + listens for the
 * `cloud-profile-changed` window event so a switch in Settings updates
 * the SyncIndicator badge live.
 */
export function useActiveProfile(): CloudProfile | null {
  const [active, setActive] = useState<CloudProfile | null>(null);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      invoke<CloudProfile>('get_active_cloud_profile')
        .then((p) => {
          if (!cancelled) setActive(p);
        })
        .catch(() => {
          if (!cancelled) setActive(null);
        });
    };
    refresh();
    const handler = () => refresh();
    window.addEventListener('cloud-profile-changed', handler);
    return () => {
      cancelled = true;
      window.removeEventListener('cloud-profile-changed', handler);
    };
  }, []);

  return active;
}
