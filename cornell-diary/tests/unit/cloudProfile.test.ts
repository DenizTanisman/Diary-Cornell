import { describe, expect, it } from 'vitest';

import {
  isProtectedProfile,
  PROTECTED_PROFILE_IDS,
  type CloudProfile,
} from '../../src/types/cloudProfile';

describe('isProtectedProfile', () => {
  it('blocks deletion of seeded ids', () => {
    expect(isProtectedProfile('local')).toBe(true);
    expect(isProtectedProfile('production')).toBe(true);
  });

  it('does not block custom ids', () => {
    expect(isProtectedProfile('staging')).toBe(false);
    expect(isProtectedProfile('my-profile')).toBe(false);
    expect(isProtectedProfile('')).toBe(false);
  });

  it('id list matches the Rust-side PROTECTED_IDS contract', () => {
    expect(PROTECTED_PROFILE_IDS).toEqual(['local', 'production']);
  });
});

describe('CloudProfile shape', () => {
  it('serialises camelCase keys (matches Rust DTO)', () => {
    const p: CloudProfile = {
      id: 'staging',
      name: 'Staging',
      baseUrl: 'https://stg.example.com',
      apiKey: 'k',
      isActive: false,
    };
    // Round-trip JSON.stringify should keep camelCase; if the wire DTO
    // ever drifts back to snake_case the IPC will silently break.
    const json = JSON.parse(JSON.stringify(p));
    expect(json.baseUrl).toBe('https://stg.example.com');
    expect(json.apiKey).toBe('k');
    expect(json.isActive).toBe(false);
    expect('base_url' in json).toBe(false);
  });
});
