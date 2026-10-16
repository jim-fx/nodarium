import { describe, expect, it } from 'vitest';
import { formatRelativeTime } from './formatRelativeTime';

const now = new Date('2026-10-16T12:00:00Z');
const ago = (ms: number) => new Date(now.getTime() - ms).toISOString();

describe('formatRelativeTime', () => {
  it('handles missing and invalid dates', () => {
    expect(formatRelativeTime(undefined, now)).toBe('');
    expect(formatRelativeTime('not a date', now)).toBe('');
  });

  it('formats recent dates relatively', () => {
    expect(formatRelativeTime(ago(10 * 1000), now)).toBe('just now');
    expect(formatRelativeTime(ago(5 * 60 * 1000), now)).toBe('5m ago');
    expect(formatRelativeTime(ago(3 * 60 * 60 * 1000), now)).toBe('3h ago');
    expect(formatRelativeTime(ago(30 * 60 * 60 * 1000), now)).toBe('yesterday');
    expect(formatRelativeTime(ago(3 * 24 * 60 * 60 * 1000), now)).toBe('3d ago');
  });

  it('falls back to a date for older entries', () => {
    expect(formatRelativeTime('2026-10-02T12:00:00Z', now)).toBe('Oct 2');
    expect(formatRelativeTime('2024-10-02T12:00:00Z', now)).toBe('Oct 2, 2024');
  });
});
