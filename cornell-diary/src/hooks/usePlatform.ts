import { useMemo } from 'react';
import { platform } from '@tauri-apps/plugin-os';

export type Platform = 'macos' | 'linux' | 'windows' | 'android' | 'ios' | 'unknown';

export interface PlatformInfo {
  platform: Platform;
  /** True for android + ios — phones/tablets where Cloud-spawn UI is
   *  irrelevant and tap targets / layouts may diverge from desktop. */
  isMobile: boolean;
}

/** Read the host OS via @tauri-apps/plugin-os. Tauri 2's `platform()`
 *  is synchronous (it caches the value at plugin init), so a useMemo
 *  is enough — no async dance, no flicker. Wrapped in try/catch so
 *  vitest / browser preview (where the plugin isn't loaded) falls
 *  back to 'unknown' and isMobile=false. */
export function usePlatform(): PlatformInfo {
  return useMemo<PlatformInfo>(() => {
    try {
      const p = platform() as Platform;
      return {
        platform: p,
        isMobile: p === 'android' || p === 'ios',
      };
    } catch {
      return { platform: 'unknown', isMobile: false };
    }
  }, []);
}
