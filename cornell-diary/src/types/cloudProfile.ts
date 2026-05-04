/**
 * Wire shape for `cloud_profiles` rows. Mirrors the Rust struct in
 * `src-tauri/src/db/cloud_profile.rs` (camelCase via #[serde(rename_all)]).
 */
export interface CloudProfile {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string | null;
  isActive: boolean;
}

export const PROTECTED_PROFILE_IDS = ['local', 'production'] as const;

export function isProtectedProfile(id: string): boolean {
  return (PROTECTED_PROFILE_IDS as readonly string[]).includes(id);
}
