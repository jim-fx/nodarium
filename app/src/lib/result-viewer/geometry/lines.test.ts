import { sortResultItems } from '@nodarium/runtime';
import { Group, LineSegments } from 'three';
import { describe, expect, it } from 'vitest';
import { createResultPools } from './geometryPool';
import { groupColor, LINES_TAG, readLines } from './lines';

// two line segments, the first in group 0, the second in group 3
function lines() {
  const data = new Int32Array(2 + 2 * 7);
  data.set([LINES_TAG, 2]);
  const floats = new Float32Array(data.buffer);
  floats.set([0, 0, 0, 0, 1, 0], 2);
  floats.set([0, 1, 0, 1, 2, 0], 9);
  data[8] = 0;
  data[15] = 3;
  return data;
}

describe('readLines', () => {
  it('reads the ends and the group of every segment', () => {
    const { positions, groups } = readLines(lines());
    expect(positions).toEqual([0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 2, 0]);
    expect(groups).toEqual([0, 3]);
  });

  it('rejects data of the wrong length', () => {
    expect(() => readLines(lines().subarray(0, 10))).toThrow(/need 16 words/);
  });
});

describe('groupColor', () => {
  it('gives neighbouring groups different colors that stay the same', () => {
    expect(groupColor(1).equals(groupColor(1))).toBe(true);
    expect(groupColor(0).equals(groupColor(1))).toBe(false);
    expect(groupColor(1).equals(groupColor(2))).toBe(false);
    for (const c of [groupColor(0), groupColor(7), groupColor(123456)]) {
      for (const v of [c.r, c.g, c.b]) {
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      }
    }
  });
});

describe('result lines', () => {
  it('are sorted out of a result and shown colored by group', () => {
    const item = lines();
    expect(sortResultItems([item]).lines).toEqual([item]);

    const scene = new Group();
    const stats = createResultPools(scene, undefined as never).update([item]);
    expect(stats.totalVertices).toBe(4);
    expect(stats.bounds.max.y).toBe(2);
    const shown = scene.children
      .flatMap((pool) => pool.children)
      .find((c): c is LineSegments => c instanceof LineSegments)!;
    const colors = shown.geometry.getAttribute('color');
    const expectColor = (vertex: number, group: number) => {
      const expected = groupColor(group);
      expect(colors.getX(vertex)).toBeCloseTo(expected.r);
      expect(colors.getY(vertex)).toBeCloseTo(expected.g);
      expect(colors.getZ(vertex)).toBeCloseTo(expected.b);
    };
    expectColor(0, 0);
    expectColor(2, 3);
  });
});
