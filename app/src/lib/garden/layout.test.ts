import { describe, expect, it } from 'vitest';
import { BED_DEPTH, BED_MARGIN, BED_WIDTH, PLANT_ISLAND_RADIUS } from './config';
import { layoutPlants } from './layout';
import { OBSTACLES } from './zen/config';

const ids = [3, 17, 42, 108, 256, 1000, 1001, 1002, 4711, 9000, 9001, 12345];

describe('layoutPlants', () => {
  it('places every plant inside of the bed', () => {
    const positions = layoutPlants(ids, OBSTACLES);
    expect(positions.size).toBe(ids.length);
    for (const { x, z } of positions.values()) {
      expect(Math.abs(x)).toBeLessThanOrEqual(BED_WIDTH / 2 - BED_MARGIN);
      expect(Math.abs(z)).toBeLessThanOrEqual(BED_DEPTH / 2 - BED_MARGIN);
    }
  });

  it('keeps the plants away from the pond and the rocks', () => {
    for (const { x, z } of layoutPlants(ids, OBSTACLES).values()) {
      for (const o of OBSTACLES) {
        expect(Math.hypot(o.x - x, o.z - z)).toBeGreaterThanOrEqual(o.radius + PLANT_ISLAND_RADIUS);
      }
    }
  });

  it('does not depend on the order of the ids', () => {
    expect(layoutPlants([...ids].reverse(), OBSTACLES)).toEqual(layoutPlants(ids, OBSTACLES));
  });

  it('keeps existing plants in place when a newer one is added', () => {
    const before = layoutPlants(ids, OBSTACLES);
    const after = layoutPlants([...ids, 99999], OBSTACLES);
    for (const id of ids) {
      expect(after.get(id)).toEqual(before.get(id));
    }
  });
});
