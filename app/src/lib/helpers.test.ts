import { humanizeDuration, humanizeNumber } from '#lib/helpers/index.ts';
import { describe, expect, it } from 'vitest';

describe('helpers', () => {
  describe('humanizeNumber', () => {
    it('should return unchanged numbers below 1000', () => {
      expect(humanizeNumber(0)).toBe('0');
      expect(humanizeNumber(999)).toBe('999');
    });

    it('should add K suffix for thousands', () => {
      expect(humanizeNumber(1000)).toBe('1K');
      expect(humanizeNumber(1500)).toBe('1.5K');
      expect(humanizeNumber(999999)).toBe('1000K');
    });

    it('should add M suffix for millions', () => {
      expect(humanizeNumber(1000000)).toBe('1M');
      expect(humanizeNumber(2500000)).toBe('2.5M');
    });

    it('should add B suffix for billions', () => {
      expect(humanizeNumber(1000000000)).toBe('1B');
    });
  });

  describe('humanizeDuration', () => {
    it('should return ms for very short durations', () => {
      expect(humanizeDuration(100)).toBe('100ms');
      expect(humanizeDuration(999)).toBe('999ms');
    });

    it('should format seconds', () => {
      expect(humanizeDuration(1000)).toBe('1s');
      expect(humanizeDuration(1500)).toBe('1s500ms');
      expect(humanizeDuration(59000)).toBe('59s');
    });

    it('should format minutes', () => {
      expect(humanizeDuration(60000)).toBe('1m');
      expect(humanizeDuration(90000)).toBe('1m 30s');
    });

    it('should format hours', () => {
      expect(humanizeDuration(3600000)).toBe('1h');
      expect(humanizeDuration(3661000)).toBe('1h 1m 1s');
    });

    it('should format days', () => {
      expect(humanizeDuration(86400000)).toBe('1d');
      expect(humanizeDuration(90061000)).toBe('1d 1h 1m 1s');
    });

    it('should handle zero', () => {
      expect(humanizeDuration(0)).toBe('0ms');
    });
  });
});
