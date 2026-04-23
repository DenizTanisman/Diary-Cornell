import { describe, expect, it } from 'vitest';
import {
  addDaysISO,
  formatTurkishLong,
  isValidISODate,
  subDaysISO,
  toISODate,
  todayISO,
} from '../../src/utils/date';

describe('date utils', () => {
  it('validates ISO dates', () => {
    expect(isValidISODate('2026-04-23')).toBe(true);
    expect(isValidISODate('2026/04/23')).toBe(false);
    expect(isValidISODate('bad')).toBe(false);
    expect(isValidISODate('2026-13-01')).toBe(false);
  });

  it('produces ISO today', () => {
    const t = todayISO();
    expect(t).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('converts Date to ISO', () => {
    const d = new Date('2026-04-23T10:00:00Z');
    expect(toISODate(d)).toMatch(/^2026-04-2[23]$/); // timezone tolerance
  });

  it('adds and subtracts days', () => {
    expect(addDaysISO('2026-04-23', 1)).toBe('2026-04-24');
    expect(subDaysISO('2026-04-23', 2)).toBe('2026-04-21');
    expect(addDaysISO('2026-04-30', 1)).toBe('2026-05-01');
  });

  it('formats turkish long', () => {
    const s = formatTurkishLong('2026-04-23');
    expect(s).toMatch(/Nisan/);
    expect(s).toMatch(/2026/);
  });
});
