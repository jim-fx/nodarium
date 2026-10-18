import { BED_DEPTH } from '../config';
import type { StyleSettings } from '../config';

/** The raised bed stands on the lawn, its soil is at 0 */
export const GROUND_LEVEL = -0.65;

/** The brick path leads from the visitor to the front of the raised bed */
export const PATH_POINTS: [number, number][] = [
  [-1.4, 22],
  [1.2, 15],
  [-0.6, 10],
  [0.3, BED_DEPTH / 2 + 0.35]
];
export const PATH_WIDTH = 1.8;

export const COLORS = {
  sun: '#ffd9a0',
  sky: '#d9e8ff',
  grass: '#56702e',
  lawn: '#7d8f5a',
  soil: '#8a7f78',
  boards: '#c9b8a6',
  bricks: '#c9a99a',
  hedge: '#6f9150',
  tree: '#4f7a3c',
  sunlitTree: '#b9d77a',
  pollen: '#ffe9a8',
  haze: '#cfdca4'
};

export const COTTAGE: StyleSettings = {
  plant: '#5e9a35',
  cushion: false,
  fog: { start: 14, length: 60 },
  aperture: { overview: 0.0011, focus: 0.003 },
  maxBlur: 0.014,
  bloom: 0.25,
  vignette: 0.45
};
