import { describe, expect, it } from 'vitest';
import { sanitizeText, sanitizeTitle } from '../../src/utils/sanitize';

describe('sanitize', () => {
  it('truncates long text', () => {
    const big = 'a'.repeat(500);
    expect(sanitizeText(big, 100)).toHaveLength(100);
  });

  it('returns empty for non-string', () => {
    // @ts-expect-error intentional bad input
    expect(sanitizeText(undefined)).toBe('');
  });

  it('strips newlines from titles and trims', () => {
    expect(sanitizeTitle('  Hello\nworld  ')).toBe('Hello world');
  });
});
