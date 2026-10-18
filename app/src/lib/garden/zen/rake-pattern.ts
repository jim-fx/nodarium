import { BED_DEPTH, BED_WIDTH } from '../config';
import type { Island } from '../layout';
import { RAKE_RINGS, RAKE_SPACING } from './config';

/**
 * Height of a rake groove profile, 0 in the groove and 1 on the ridge.
 * The ridges are a bit wider than the grooves, like sand pushed aside by a rake.
 */
function profile(phase: number) {
  const wave = 0.5 - 0.5 * Math.cos(phase * Math.PI * 2);
  return Math.sqrt(wave);
}

/**
 * Renders the height map of the raked sand.
 * Rings are drawn around every island, the rest of the bed is raked in
 * straight lines along its width, which stop at the outermost ring.
 * Row 0 is the front of the bed (z = BED_DEPTH / 2).
 */
export function renderRakePattern(islands: Island[], pixelsPerUnit = 96) {
  const width = Math.round(BED_WIDTH * pixelsPerUnit);
  const height = Math.round(BED_DEPTH * pixelsPerUnit);
  const data = new Uint8Array(width * height);
  const ringBand = RAKE_RINGS * RAKE_SPACING;

  for (let j = 0; j < height; j++) {
    const z = BED_DEPTH / 2 - ((j + 0.5) / height) * BED_DEPTH;
    for (let i = 0; i < width; i++) {
      const x = ((i + 0.5) / width) * BED_WIDTH - BED_WIDTH / 2;

      // distance to the edge of the closest island
      let edge = Infinity;
      for (const island of islands) {
        edge = Math.min(edge, Math.hypot(x - island.x, z - island.z) - island.radius);
      }

      let value: number;
      if (edge < 0) {
        // the flat sand an island sits on
        value = 0.55;
      } else if (edge < ringBand) {
        value = profile(edge / RAKE_SPACING);
      } else {
        value = profile(z / RAKE_SPACING);
      }
      data[j * width + i] = Math.round(value * 255);
    }
  }

  return { data, width, height };
}
