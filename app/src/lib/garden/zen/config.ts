import type { StyleSettings } from '../config';
import type { Island } from '../layout';

/** Height of the forest floor around the bed, the sand is at 0 */
export const FLOOR_LEVEL = -0.4;

/** Distance between two rake grooves */
export const RAKE_SPACING = 0.16;
/** Number of rings raked around every island */
export const RAKE_RINGS = 4;

export const POND = { x: 5.1, z: -1.9, radius: 1.55 };

export interface RockPlacement {
  x: number;
  z: number;
  /** footprint radius on the sand */
  radius: number;
  height: number;
  rotation: number;
  seed: number;
}

/** A classic asymmetric composition, one group of three and one pair */
export const ROCKS: RockPlacement[] = [
  { x: -4.6, z: -2.2, radius: 0.85, height: 1.25, rotation: 0.4, seed: 11 },
  { x: -3.55, z: -1.55, radius: 0.5, height: 0.6, rotation: 1.9, seed: 23 },
  { x: -4.1, z: -1.0, radius: 0.32, height: 0.3, rotation: 2.7, seed: 37 },
  { x: 2.6, z: 2.5, radius: 0.6, height: 0.55, rotation: 0.9, seed: 51 },
  { x: 3.35, z: 2.95, radius: 0.36, height: 0.32, rotation: 2.2, seed: 67 }
];

/** Everything plants have to keep their distance from */
export const OBSTACLES: Island[] = [
  { x: POND.x, z: POND.z, radius: POND.radius + 0.3 },
  ...ROCKS.map(({ x, z, radius }) => ({ x, z, radius }))
];

export const COLORS = {
  // layer-0 of the default dark theme
  background: '#060606',
  floor: '#383838',
  sand: '#c8c8c4',
  stone: '#c4c4c4',
  moss: '#a9b886',
  wood: '#9a9a9a',
  water: '#141a1a',
  light: '#fff3e6'
};

export const ZEN: StyleSettings = {
  // the average color of the matcap the plants are shown with in the editor
  plant: '#6f8a4a',
  cushion: true,
  fog: { start: 4, length: 22 },
  aperture: { overview: 0.0005, focus: 0.0025 },
  maxBlur: 0.008,
  bloom: 0,
  vignette: 0.5
};
