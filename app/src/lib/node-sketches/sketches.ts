/** A point in a panel of a sketch, 0 to 1 from the left and from the bottom */
export type SketchPoint = [number, number];

/** `accent` is what the node changes, `base` what it keeps, `faint` the frame around it */
export type SketchTone = 'base' | 'accent' | 'faint';

export type SketchShape =
  | { kind: 'line'; points: SketchPoint[]; tone: SketchTone; width?: number }
  | { kind: 'dot'; at: SketchPoint; tone: SketchTone; r?: number };

export type SketchFrame = { input: SketchShape[]; output: SketchShape[] };

export type NodeSketch = {
  /** the type of the data in each panel, shown under it */
  input: string;
  output: string;
  /**
   * `p` goes from 0 to 1 and back with a rest at both ends, `cycle` counts the times it was back
   * at 0, for a sketch that draws new values every time
   */
  draw: (p: number, cycle: number) => SketchFrame;
};

/** One loop: from 0 up, a rest at 1, down to 0, a rest at 0 */
export const SKETCH_PERIOD = 4000;

const ease = (x: number) => x * x * (3 - 2 * x);

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
function hash(seed: number) {
  const x = Math.sin(seed * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

function sample(n: number, f: (t: number) => SketchPoint): SketchPoint[] {
  return Array.from({ length: n + 1 }, (_, i) => f(i / n));
}

/** The guide of `strands` and the stem of `branch`: a path that leans a little to the right */
const stemAt = (t: number): SketchPoint => [0.45 + 0.08 * t * t, 0.08 + 0.84 * t];

function stemTangent(t: number): SketchPoint {
  const [x0, y0] = stemAt(Math.max(0, t - 0.01));
  const [x1, y1] = stemAt(Math.min(1, t + 0.01));
  const l = Math.hypot(x1 - x0, y1 - y0);
  return [(x1 - x0) / l, (y1 - y0) / l];
}

const stem = (tone: SketchTone): SketchShape => ({
  kind: 'line',
  points: sample(16, stemAt),
  tone,
  width: 2
});

/** The default of a new curve node rises slowly at first and steeply at the end */
const curveAt = (x: number) => x * x * (3 - 2 * x) * 0.6 + x * x * 0.4;

const axes: SketchShape = {
  kind: 'line',
  points: [[0.12, 0.92], [0.12, 0.1], [0.92, 0.1]],
  tone: 'faint'
};

function numberLine(at: number, tone: SketchTone): SketchShape[] {
  return [
    { kind: 'line', points: [[0.1, 0.5], [0.9, 0.5]], tone: 'faint' },
    { kind: 'line', points: [[0.1, 0.42], [0.1, 0.58]], tone: 'faint' },
    { kind: 'line', points: [[0.9, 0.42], [0.9, 0.58]], tone: 'faint' },
    { kind: 'dot', at: [lerp(0.1, 0.9, at), 0.5], tone, r: 3 }
  ];
}

const curve: NodeSketch = {
  input: 'float',
  output: 'float',
  draw: (p) => {
    const x = lerp(0.12, 0.92, p);
    const y = lerp(0.1, 0.92, curveAt(p));
    return {
      input: numberLine(p, 'base'),
      output: [
        axes,
        {
          kind: 'line',
          points: sample(20, (t) => [lerp(0.12, 0.92, t), lerp(0.1, 0.92, curveAt(t))]),
          tone: 'base'
        },
        { kind: 'line', points: [[x, 0.1], [x, y], [0.12, y]], tone: 'faint' },
        { kind: 'dot', at: [x, y], tone: 'base', r: 2 },
        { kind: 'dot', at: [0.12, y], tone: 'accent', r: 3 }
      ]
    };
  }
};

/** Each row is one place that reads the node, every place draws its own value */
const RANDOM_ROWS = 5;

const random: NodeSketch = {
  input: 'min, max',
  output: 'float',
  draw: (p, cycle) => {
    const rows: SketchShape[] = Array.from({ length: RANDOM_ROWS }, (_, i) => {
      const y = lerp(0.15, 0.85, i / (RANDOM_ROWS - 1));
      const value = lerp(0.2, 0.8, hash(cycle * RANDOM_ROWS + i));
      return { kind: 'dot', at: [lerp(0.5, value, p), y], tone: 'accent', r: 2.5 };
    });
    return {
      input: [
        { kind: 'line', points: [[0.1, 0.5], [0.9, 0.5]], tone: 'faint' },
        { kind: 'line', points: [[0.2, 0.32], [0.2, 0.68]], tone: 'base', width: 2 },
        { kind: 'line', points: [[0.8, 0.32], [0.8, 0.68]], tone: 'base', width: 2 }
      ],
      output: [
        { kind: 'line', points: [[0.2, 0.05], [0.2, 0.95]], tone: 'faint' },
        { kind: 'line', points: [[0.8, 0.05], [0.8, 0.95]], tone: 'faint' },
        ...rows
      ]
    };
  }
};

const ALPHA_PATH = (t: number): SketchPoint => [0.3 + 0.18 * Math.sin(t * 2.6), 0.08 + 0.84 * t];

const alpha: NodeSketch = {
  input: 'path',
  output: 'float',
  draw: (p) => {
    const at = ALPHA_PATH(p);
    return {
      input: [
        { kind: 'line', points: sample(16, ALPHA_PATH), tone: 'base', width: 2 },
        { kind: 'dot', at, tone: 'base', r: 3 }
      ],
      output: [
        { kind: 'line', points: [[0.5, 0.08], [0.5, 0.92]], tone: 'faint', width: 6 },
        {
          kind: 'line',
          points: [[0.5, 0.08], [0.5, lerp(0.08, 0.92, p)]],
          tone: 'accent',
          width: 6
        },
        { kind: 'line', points: [[0.62, 0.08], [0.7, 0.08]], tone: 'faint' },
        { kind: 'line', points: [[0.62, 0.92], [0.7, 0.92]], tone: 'faint' }
      ]
    };
  }
};

/** Where the branches sit along the stem, and the side they grow to */
const BRANCHES = [0.3, 0.45, 0.6, 0.75];

const branch: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const branches: SketchShape[] = BRANCHES.map((t, i) => {
      const [bx, by] = stemAt(t);
      const side = i % 2 ? 1 : -1;
      // shorter towards the tip, like the default length of a branch
      const length = 0.34 * (1 - t * 0.6) * p;
      return {
        kind: 'line',
        points: sample(6, (s) => [
          bx + side * length * s,
          by + length * (0.55 * s + 0.25 * s * s)
        ]),
        tone: 'accent',
        width: 1.5
      };
    });
    return { input: [stem('base')], output: [stem('base'), ...branches] };
  }
};

/** The strands start on the guide and spread out to the wall of its tube */
const STRANDS = [-1, -0.5, 0, 0.5, 1];
const TUBE = 0.12;

const strands: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const offset = (t: number, d: number): SketchPoint => {
      const [x, y] = stemAt(t);
      const [tx, ty] = stemTangent(t);
      return [x - ty * d, y + tx * d];
    };
    const tube: SketchShape[] = [-1, 1].map((side) => ({
      kind: 'line',
      points: sample(16, (t) => offset(t, side * TUBE)),
      tone: 'faint'
    }));
    const lines: SketchShape[] = STRANDS.map((d, i) => ({
      kind: 'line',
      points: sample(24, (t) => {
        const wobble = 0.006 * Math.sin(t * 8 + i * 1.7) * p;
        return offset(t, d * TUBE * 0.85 * p + wobble);
      }),
      tone: 'accent'
    }));
    return {
      input: [...tube, stem('base')],
      output: [...tube, ...lines]
    };
  }
};

/**
 * Only nodes whose effect is hard to guess from their name and inputs get a sketch: what a curve
 * does to a value, that random draws anew where it is read, what alpha is, how branch places
 * branches and that strands fills the tube of one path with many
 */
export const NODE_SKETCHES: Record<string, NodeSketch> = {
  'max/core/curve': curve,
  'max/core/random': random,
  'max/core/alpha': alpha,
  'max/plantarium/branch': branch,
  'max/myzel/strands': strands
};

export function sketchFor(nodeType: string): NodeSketch | undefined {
  return NODE_SKETCHES[nodeType];
}
