export type Framing = {
  center: [number, number, number];
  /** radius of the sphere around the content */
  radius: number;
  /** distance of the camera to the center that fits the sphere */
  distance: number;
  near: number;
  far: number;
};

/** Content smaller than this (in meters) is framed as if it were this big */
const MIN_RADIUS = 0.001;
/** The radius of an empty scene, one meter */
const EMPTY_RADIUS = 1;
const MARGIN = 1.25;

/**
 * Works out where the camera has to look and how far away it has to be
 * to fit a bounding box, whatever the scale of the content.
 * Returns undefined for boxes with NaN or infinite values.
 */
export function frameBounds(
  min: readonly number[],
  max: readonly number[],
  fovDegrees = 50
): Framing | undefined {
  if (![...min, ...max].every(Number.isFinite)) return undefined;

  const empty = min.some((v, i) => v > max[i]);

  const center: [number, number, number] = empty ? [0, 0.5, 0] : [
    (min[0] + max[0]) / 2,
    (min[1] + max[1]) / 2,
    (min[2] + max[2]) / 2
  ];
  const size = empty ? 0 : Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]);
  const radius = empty ? EMPTY_RADIUS : Math.max(size / 2, MIN_RADIUS);

  const distance = (radius * MARGIN) / Math.sin((fovDegrees * Math.PI) / 360);

  return {
    center,
    radius,
    distance,
    near: radius / 1000,
    far: Math.max(distance + radius * 100, 100)
  };
}

/** Frame rate independent version of lerp, `speed` is roughly 1 / seconds to arrive */
export function damp(current: number, target: number, speed: number, delta: number) {
  return target + (current - target) * Math.exp(-speed * delta);
}

/** The size of a grid section, a power of ten so the grid lines up with the units */
export function gridSectionSize(radius: number) {
  return Math.pow(10, Math.floor(Math.log10(Math.max(radius, MIN_RADIUS))));
}
