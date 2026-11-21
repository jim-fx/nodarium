/** A point in a panel of a sketch, 0 to 1 from the left and from the bottom */
export type SketchPoint = [number, number];

/** `accent` is what the node changes, `base` what it keeps, `faint` the frame around it */
export type SketchTone = 'base' | 'accent' | 'faint';

export type SketchShape =
  | { kind: 'line'; points: SketchPoint[]; tone: SketchTone; width?: number }
  | { kind: 'dot'; at: SketchPoint; tone: SketchTone; r?: number }
  | { kind: 'area'; points: SketchPoint[]; tone: SketchTone };

export type SketchFrame = { input: SketchShape[]; output: SketchShape[] };

export type NodeSketch = {
  /** the type of the data in each panel, shown under it; a node that takes no data has no input panel */
  input?: string;
  output: string;
  /**
   * `p` goes from 0 to 1 and back with a rest at both ends, `cycle` counts the times it was back
   * at 0, for a sketch that draws new values every time
   */
  draw: (p: number, cycle: number) => SketchFrame;
};

/** The size of a panel in pixels */
export const PANEL = { width: 92, height: 68 };

/** A panel is wider than high, a round shape is this much narrower in x than in y */
export const ASPECT = PANEL.height / PANEL.width;

/** One loop: from 0 up, a rest at 1, down to 0, a rest at 0 */
export const SKETCH_PERIOD = 4000;

export const ease = (x: number) => x * x * (3 - 2 * x);

/** `p` and `cycle` at `ms` milliseconds after the sketch started */
export function sketchPhase(ms: number): { p: number; cycle: number } {
  const cycle = Math.floor(ms / SKETCH_PERIOD);
  const u = (ms % SKETCH_PERIOD) / SKETCH_PERIOD;
  if (u < 0.35) return { p: ease(u / 0.35), cycle };
  if (u < 0.6) return { p: 1, cycle };
  if (u < 0.85) return { p: ease(1 - (u - 0.6) / 0.25), cycle };
  return { p: 0, cycle };
}

/** The same numbers for the same seed, so a frame doesn't flicker between two animation frames */
export function hash(seed: number) {
  const x = Math.sin(seed * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
}

export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

export const clamp = (x: number, min = 0, max = 1) => Math.min(max, Math.max(min, x));

export function sample(n: number, f: (t: number) => SketchPoint): SketchPoint[] {
  return Array.from({ length: n + 1 }, (_, i) => f(i / n));
}

export const range = (n: number) => Array.from({ length: n }, (_, i) => i);

export const line = (
  points: SketchPoint[],
  tone: SketchTone = 'base',
  width?: number
): SketchShape => ({
  kind: 'line',
  points,
  tone,
  ...(width && { width })
});

export const dot = (at: SketchPoint, tone: SketchTone = 'accent', r?: number): SketchShape => ({
  kind: 'dot',
  at,
  tone,
  ...(r && { r })
});

export const area = (points: SketchPoint[], tone: SketchTone = 'faint'): SketchShape => ({
  kind: 'area',
  points,
  tone
});

/** A closed ellipse, `wobble` moves each point by the angle */
export function ellipse(
  [cx, cy]: SketchPoint,
  rx: number,
  ry: number,
  n = 36,
  wobble?: (angle: number) => SketchPoint
): SketchPoint[] {
  return sample(n, (t) => {
    const a = t * Math.PI * 2;
    const [dx, dy] = wobble?.(a) ?? [0, 0];
    return [cx + Math.cos(a) * rx + dx, cy + Math.sin(a) * ry + dy];
  });
}

/** A circle that looks round in a panel */
export const circle = (center: SketchPoint, r: number, n = 36) => ellipse(center, r * ASPECT, r, n);

/**
 * A path that starts at `start` and turns by `angle(t)` along its length, 0 is straight up and a
 * positive angle turns to the right. The y of a step is corrected by the aspect, so the length
 * looks the same in every direction
 */
export function walk(
  [x, y]: SketchPoint,
  length: number,
  angle: (t: number) => number,
  n = 24
): SketchPoint[] {
  const points: SketchPoint[] = [[x, y]];
  const step = length / n;
  for (let i = 0; i < n; i++) {
    const a = angle((i + 0.5) / n);
    x += Math.sin(a) * step * ASPECT;
    y += Math.cos(a) * step;
    points.push([x, y]);
  }
  return points;
}

/** The unit normal of a polyline at point `i`, to the left of its direction */
function normalAt(points: SketchPoint[], i: number): SketchPoint {
  const [x0, y0] = points[Math.max(0, i - 1)];
  const [x1, y1] = points[Math.min(points.length - 1, i + 1)];
  const l = Math.hypot(x1 - x0, y1 - y0) || 1;
  return [-(y1 - y0) / l, (x1 - x0) / l];
}

/** The polyline moved sideways by `d(t)`, t from 0 at its start to 1 at its end */
export function offset(points: SketchPoint[], d: (t: number) => number): SketchPoint[] {
  return points.map(([x, y], i) => {
    const [nx, ny] = normalAt(points, i);
    const o = d(i / (points.length - 1));
    return [x + nx * o * ASPECT, y + ny * o];
  });
}

/** The outline of a band of width `w(t)` around a polyline, for a tube, a blade or a skin */
export function band(points: SketchPoint[], w: (t: number) => number): SketchPoint[] {
  return [...offset(points, (t) => w(t) / 2), ...offset(points, (t) => -w(t) / 2).reverse()];
}

/** A point `t` of the way along a polyline, by its points */
export function along(points: SketchPoint[], t: number): SketchPoint {
  const f = clamp(t) * (points.length - 1);
  const i = Math.min(points.length - 2, Math.floor(f));
  const [x0, y0] = points[i];
  const [x1, y1] = points[i + 1];
  return [lerp(x0, x1, f - i), lerp(y0, y1, f - i)];
}

/** The polyline up to `t` of its length, by its points */
export function upTo(points: SketchPoint[], t: number): SketchPoint[] {
  const f = clamp(t) * (points.length - 1);
  return [...points.slice(0, Math.floor(f) + 1), along(points, t)];
}

/** A straight stem from the bottom to the top of a panel */
export const upright = (x = 0.5, from = 0.08, to = 0.92): SketchPoint[] =>
  sample(24, (t) => [x, lerp(from, to, t)]);

/** The guide of `strands` and the stem of `branch`: a path that leans a little to the right */
export const stemAt = (t: number): SketchPoint => [0.45 + 0.08 * t * t, 0.08 + 0.84 * t];

export function stemTangent(t: number): SketchPoint {
  const [x0, y0] = stemAt(Math.max(0, t - 0.01));
  const [x1, y1] = stemAt(Math.min(1, t + 0.01));
  const l = Math.hypot(x1 - x0, y1 - y0);
  return [(x1 - x0) / l, (y1 - y0) / l];
}

export const stem = (tone: SketchTone): SketchShape => ({
  kind: 'line',
  points: sample(16, stemAt),
  tone,
  width: 2
});

/** A leaf from its base, pointing along `angle` (0 up, positive to the right) */
export function leaf(base: SketchPoint, angle: number, size: number): SketchPoint[] {
  const midrib = walk(base, size, () => angle, 10);
  return band(midrib, (t) => size * 0.55 * Math.sin(Math.PI * Math.pow(t, 0.8)));
}

/** A number line across the middle of a panel, with a dot at `at` from 0 to 1 */
export function numberLine(at: number, tone: SketchTone, y = 0.5): SketchShape[] {
  return [
    line([[0.1, y], [0.9, y]], 'faint'),
    line([[0.1, y - 0.08], [0.1, y + 0.08]], 'faint'),
    line([[0.9, y - 0.08], [0.9, y + 0.08]], 'faint'),
    dot([lerp(0.1, 0.9, at), y], tone, 3)
  ];
}

export const axes: SketchShape = line([[0.12, 0.92], [0.12, 0.1], [0.92, 0.1]], 'faint');

/** A value from 0 to 1 read off the graph of `f`: the graph, the guide and the value on the y axis */
export function graphReading(f: (x: number) => number, p: number): SketchShape[] {
  const x = lerp(0.12, 0.92, p);
  const y = lerp(0.1, 0.92, f(p));
  return [
    axes,
    line(sample(40, (t) => [lerp(0.12, 0.92, t), lerp(0.1, 0.92, f(t))])),
    line([[x, 0.1], [x, y], [0.12, y]], 'faint'),
    dot([x, y], 'base', 2),
    dot([0.12, y], 'accent', 3)
  ];
}

/** A vertical meter filled from 0 to `value` */
export function meter(value: number, x = 0.5): SketchShape[] {
  return [
    line([[x, 0.08], [x, 0.92]], 'faint', 6),
    line([[x, 0.08], [x, lerp(0.08, 0.92, value)]], 'accent', 6),
    line([[x + 0.12, 0.08], [x + 0.2, 0.08]], 'faint'),
    line([[x + 0.12, 0.92], [x + 0.2, 0.92]], 'faint')
  ];
}
