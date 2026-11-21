import {
  axes,
  dot,
  graphReading,
  hash,
  lerp,
  line,
  type NodeSketch,
  numberLine,
  sample,
  type SketchPoint,
  type SketchShape
} from './kit';

/** The default of a new curve node rises slowly at first and steeply at the end */
const curveAt = (x: number) => x * x * (3 - 2 * x) * 0.6 + x * x * 0.4;

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

/** The whole numbers that round puts its value on */
const TICKS = [0.2, 0.35, 0.5, 0.65, 0.8];
const nearestTick = (v: number) =>
  TICKS.reduce((best, t) => (Math.abs(t - v) < Math.abs(best - v) ? t : best));
const ticks = (y: number, h: number): SketchShape[] =>
  TICKS.map((x) => line([[x, y - h], [x, y + h]], 'faint'));

const round: NodeSketch = {
  input: 'float',
  output: 'integer',
  draw: (p, cycle) => {
    const v = lerp(0.22, 0.78, hash(cycle + 300));
    return {
      input: [
        line([[0.1, 0.5], [0.9, 0.5]], 'faint'),
        ...ticks(0.5, 0.06),
        dot([v, 0.5], 'base', 3)
      ],
      output: [
        line([[0.1, 0.5], [0.9, 0.5]], 'faint'),
        ...ticks(0.5, 0.06),
        dot([v, 0.5], 'faint', 3),
        dot([lerp(v, nearestTick(v), p), 0.5], 'accent', 3)
      ]
    };
  }
};

/** Three times the profile while the value goes from 0 to 1, 0 on a crest */
const waveAt = (x: number) => (1 - Math.cos(x * Math.PI * 2 * 3)) / 2;

const wave: NodeSketch = {
  input: 'float',
  output: 'float',
  draw: (p) => ({ input: numberLine(p, 'base'), output: graphReading(waveAt, p) })
};

export const CORE_SKETCHES: Record<string, NodeSketch> = {
  'max/core/curve': curve,
  'max/core/alpha': alpha,
  'max/core/round': round,
  'max/core/wave': wave
};
