import { clone, debounce, humanizeDuration, humanizeNumber, lerp, snapToGrid } from '$lib/helpers';
import { describe, expect, it } from 'vitest';

describe('helpers', () => {
  describe('snapToGrid', () => {
    it('should snap to nearest grid point', () => {
      expect(snapToGrid(5, 10)).toBe(10);
      expect(snapToGrid(15, 10)).toBe(20);
      expect(snapToGrid(0, 10)).toBe(0);
      expect(snapToGrid(-10, 10)).toBe(-10);
    });

    it('should snap exact midpoint values', () => {
      expect(snapToGrid(5, 10)).toBe(10);
    });

    it('should use default grid size of 10', () => {
      expect(snapToGrid(5)).toBe(10);
      expect(snapToGrid(15)).toBe(20);
    });

    it('should handle values exactly on grid', () => {
      expect(snapToGrid(10, 10)).toBe(10);
      expect(snapToGrid(20, 10)).toBe(20);
    });
  });

  describe('lerp', () => {
    it('should linearly interpolate between two values', () => {
      expect(lerp(0, 100, 0)).toBe(0);
      expect(lerp(0, 100, 0.5)).toBe(50);
      expect(lerp(0, 100, 1)).toBe(100);
    });

    it('should handle negative values', () => {
      expect(lerp(-50, 50, 0.5)).toBe(0);
      expect(lerp(-100, 0, 0.5)).toBe(-50);
    });

    it('should handle t values outside 0-1 range', () => {
      expect(lerp(0, 100, -0.5)).toBe(-50);
      expect(lerp(0, 100, 1.5)).toBe(150);
    });
  });

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

  describe('debounce', () => {
    it('should return a function', () => {
      const fn = debounce(() => {}, 100);
      expect(typeof fn).toBe('function');
    });

    it('should only call once when invoked multiple times within delay', () => {
      let callCount = 0;
      const fn = debounce(() => {
        callCount++;
      }, 100);
      fn();
      const firstCall = callCount;
      fn();
      fn();
      expect(callCount).toBe(firstCall);
    });
  });

  describe('clone', () => {
    it('should deep clone objects', () => {
      const original = { a: 1, b: { c: 2 } };
      const cloned = clone(original);

      expect(cloned).toEqual(original);
      expect(cloned).not.toBe(original);
      expect(cloned.b).not.toBe(original.b);
    });

    it('should handle arrays', () => {
      const original = [1, 2, [3, 4]];
      const cloned = clone(original);

      expect(cloned).toEqual(original);
      expect(cloned).not.toBe(original);
      expect(cloned[2]).not.toBe(original[2]);
    });

    it('should handle primitives', () => {
      expect(clone(42)).toBe(42);
      expect(clone('hello')).toBe('hello');
      expect(clone(true)).toBe(true);
      expect(clone(null)).toBe(null);
    });
  });
});
