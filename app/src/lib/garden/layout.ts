import { BED_DEPTH, BED_MARGIN, BED_WIDTH, PLANT_ISLAND_RADIUS, PLANT_SPACING } from './config';
import { createRandom } from './random';

export interface Island {
  x: number;
  z: number;
  radius: number;
}

const ATTEMPTS = 80;
const MIN_SPACING = 1.2;

/**
 * Scatters the plants over the bed.
 * Every plant gets its own random sequence seeded by its id, plants are placed
 * in order of their ids, so existing plants keep their spot when new ones are added.
 */
export function layoutPlants(ids: number[], obstacles: Island[] = []) {
  const positions = new Map<number, { x: number; z: number }>();
  const placed: { x: number; z: number }[] = [];

  const halfWidth = BED_WIDTH / 2 - BED_MARGIN;
  const halfDepth = BED_DEPTH / 2 - BED_MARGIN;

  for (const id of [...ids].sort((a, b) => a - b)) {
    const random = createRandom(id);
    let best = { x: 0, z: 0 };
    let bestClearance = -Infinity;

    for (
      let spacing = PLANT_SPACING;
      spacing >= MIN_SPACING && bestClearance < 0;
      spacing *= 0.85
    ) {
      for (let i = 0; i < ATTEMPTS; i++) {
        const candidate = {
          x: (random() * 2 - 1) * halfWidth,
          z: (random() * 2 - 1) * halfDepth
        };
        let clearance = Infinity;
        for (const p of placed) {
          clearance = Math.min(
            clearance,
            Math.hypot(p.x - candidate.x, p.z - candidate.z) - spacing
          );
        }
        for (const o of obstacles) {
          clearance = Math.min(
            clearance,
            Math.hypot(o.x - candidate.x, o.z - candidate.z) - o.radius - PLANT_ISLAND_RADIUS
          );
        }
        if (clearance > bestClearance) {
          best = candidate;
          bestClearance = clearance;
        }
        if (clearance >= 0) break;
      }
    }

    placed.push(best);
    positions.set(id, best);
  }

  return positions;
}
