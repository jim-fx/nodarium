import { describe, expect, it } from 'vitest';
import { curvePath, evaluateCurve } from './curve';

describe('curve', () => {
  it('should be a straight line through two points', () => {
    expect(evaluateCurve([0, 0, 1, 1], 0.25)).toBe(0.25);
    // every point of the path, control points included, lies on the line
    const coordinates = curvePath([0, 0, 1, 1]).replace(/[MC]/g, '').trim().split(/\s+/).map(
      Number
    );
    expect(coordinates).toHaveLength(8);
    for (let i = 0; i < coordinates.length; i += 2) {
      expect(coordinates[i] + coordinates[i + 1]).toBeCloseTo(100);
    }
  });

  // the same values as the tests in packages/utils/src/expr.rs
  it('should match the curve node', () => {
    const bell = [0, 0, 0.5, 1, 1, 0];
    expect(evaluateCurve(bell, 0.5)).toBe(1);
    expect(evaluateCurve(bell, 0.25)).toBe(0.625);
  });

  it('should not overshoot the points', () => {
    const plateau = [0, 0, 0.3, 1, 0.7, 1, 1, 0];
    for (let i = 0; i <= 100; i++) {
      const y = evaluateCurve(plateau, i / 100);
      expect(y).toBeGreaterThanOrEqual(0);
      expect(y).toBeLessThanOrEqual(1);
    }
    expect(evaluateCurve(plateau, 0.5)).toBe(1);
  });
});
