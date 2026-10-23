import { describe, expect, it } from 'vitest';
import { damp, frameBounds, gridSectionSize } from './camera-framing';

describe('frameBounds', () => {
  it('moves further away for a portrait viewport so the sides stay in view', () => {
    const wide = frameBounds([0, 0, 0], [1, 1, 1], 50, 2)!;
    const square = frameBounds([0, 0, 0], [1, 1, 1], 50, 1)!;
    const portrait = frameBounds([0, 0, 0], [1, 1, 1], 50, 0.5)!;
    expect(wide.distance).toBeCloseTo(square.distance);
    expect(portrait.distance).toBeGreaterThan(square.distance);
  });

  it('centers the box', () => {
    const f = frameBounds([-1, 0, -1], [1, 10, 1])!;
    expect(f.center).toEqual([0, 5, 0]);
  });

  it('does not clamp big content', () => {
    const f = frameBounds([0, 0, 0], [0, 40, 0])!;
    expect(f.center[1]).toBe(20);
    expect(f.distance).toBeGreaterThan(f.radius);
  });

  it('scales the distance and clipping planes with the content', () => {
    for (const size of [0.005, 0.2, 5, 50]) {
      const f = frameBounds([0, 0, 0], [size, size, size])!;
      expect(f.distance).toBeGreaterThan(f.radius);
      expect(f.near).toBeLessThan(f.distance - f.radius);
      expect(f.far).toBeGreaterThan(f.distance + f.radius);
    }
  });

  it('keeps the depth range the same for every scale, so tiny models do not z-fight', () => {
    const ratio = (size: number) => {
      const f = frameBounds([0, 0, 0], [size, size, size])!;
      return f.far / f.near;
    };

    for (const size of [0.001, 0.005, 0.2, 5, 5000]) {
      // a 24 bit depth buffer resolves about 1e5 well
      expect(ratio(size)).toBeLessThan(2e5);
      expect(ratio(size)).toBeCloseTo(ratio(1), 0);
    }
  });

  it('has a minimum radius for flat or single point content', () => {
    const f = frameBounds([1, 1, 1], [1, 1, 1])!;
    expect(f.radius).toBeGreaterThan(0);
    expect(f.near).toBeGreaterThan(0);
  });

  it('frames an empty box as one meter', () => {
    const f = frameBounds([Infinity, Infinity, Infinity].map(() => 1), [-1, -1, -1])!;
    expect(f.radius).toBe(1);
  });

  it('rejects NaN and infinite bounds', () => {
    expect(frameBounds([NaN, 0, 0], [1, 1, 1])).toBeUndefined();
    expect(frameBounds([-Infinity, 0, 0], [1, 1, 1])).toBeUndefined();
  });
});

describe('damp', () => {
  it('moves towards the target and arrives', () => {
    expect(damp(0, 10, 5, 0.1)).toBeGreaterThan(0);
    expect(damp(0, 10, 5, 10)).toBeCloseTo(10);
  });

  it('is independent of the frame rate', () => {
    let a = 0;
    for (let i = 0; i < 10; i++) a = damp(a, 1, 4, 0.1);
    expect(a).toBeCloseTo(damp(0, 1, 4, 1));
  });
});

describe('gridSectionSize', () => {
  it('is a power of ten that follows the content', () => {
    expect(gridSectionSize(4)).toBe(1);
    expect(gridSectionSize(0.05)).toBeCloseTo(0.01);
    expect(gridSectionSize(30)).toBe(10);
  });

  it('survives zero', () => {
    expect(gridSectionSize(0)).toBeGreaterThan(0);
  });
});
