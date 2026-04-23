import { describe, expect, it } from 'vitest';
import { sha256, verifyChecksum } from '../../src/utils/crypto';

describe('crypto', () => {
  it('produces sha256: prefixed hex digest', async () => {
    const h = await sha256('hello');
    expect(h).toMatch(/^sha256:[a-f0-9]{64}$/);
    // Known SHA-256 of "hello"
    expect(h).toBe(
      'sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824',
    );
  });

  it('verifies checksum round-trip', async () => {
    const data = 'some payload';
    const h = await sha256(data);
    expect(await verifyChecksum(data, h)).toBe(true);
    expect(await verifyChecksum(data + 'x', h)).toBe(false);
  });
});
