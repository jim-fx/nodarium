import { describe, expect, it, vi } from 'vitest';
import { clone, debounce, distanceFromPointToSegment, lerp, snapToGrid } from './index.ts';

describe('helpers', () => {
  describe('snapToGrid', () => {
    it('should snap to nearest grid point', () => {
      expect(snapToGrid(5, 10)).toBe(10);
      expect(snapToGrid(15, 10)).toBe(20);
      expect(snapToGrid(0, 10)).toBe(0);
      expect(snapToGrid(-10, 10)).toBe(-10);
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

  describe('debounce', () => {
    it('should call once after the last of several calls within the delay', () => {
      vi.useFakeTimers();
      let callCount = 0;
      const fn = debounce(() => {
        callCount++;
      }, 100);
      fn();
      vi.advanceTimersByTime(50);
      fn();
      fn();
      vi.advanceTimersByTime(99);
      expect(callCount).toBe(0);
      vi.advanceTimersByTime(1);
      expect(callCount).toBe(1);
      vi.useRealTimers();
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

  describe('distanceFromPointToSegment', () => {
    it('should measure perpendicular distance to the inside of the segment', () => {
      expect(distanceFromPointToSegment(0, 0, 10, 0, 5, 3)).toBe(3);
      expect(distanceFromPointToSegment(0, 0, 0, 10, -4, 5)).toBe(4);
    });

    it('should clamp to the nearest endpoint outside the segment', () => {
      expect(distanceFromPointToSegment(0, 0, 10, 0, -3, 4)).toBe(5);
      expect(distanceFromPointToSegment(0, 0, 10, 0, 13, 4)).toBe(5);
    });

    it('should be zero for a point on the segment', () => {
      expect(distanceFromPointToSegment(0, 0, 10, 10, 5, 5)).toBe(0);
    });

    it('should measure to the point when the segment has no length', () => {
      expect(distanceFromPointToSegment(2, 2, 2, 2, 5, 6)).toBe(5);
      expect(distanceFromPointToSegment(2, 2, 2, 2, 2, 2)).toBe(0);
    });
  });
});
