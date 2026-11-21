import {
  axes,
  dot,
  graphReading,
  hash,
  lerp,
  line,
  meter,
  type NodeSketch,
  numberLine,
  range,
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

/** Each row is one place that reads the node, every place draws its own value */
const RANDOM_ROWS = 5;
const rowY = (i: number) => lerp(0.15, 0.85, i / (RANDOM_ROWS - 1));

const random: NodeSketch = {
  input: 'min, max',
  output: 'float',
  draw: (p, cycle) => {
    const rows: SketchShape[] = Array.from({ length: RANDOM_ROWS }, (_, i) => {
      const y = rowY(i);
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

const math: NodeSketch = {
  input: 'a, b',
  output: 'float',
  draw: (p) => {
    const [a, b] = [0.3, 0.35];
    const x = (v: number) => lerp(0.1, 0.9, v);
    return {
      input: [...numberLine(a, 'base', 0.7), ...numberLine(b, 'base', 0.3)],
      output: [
        ...numberLine(a + b * p, 'accent'),
        line([[x(0), 0.5], [x(a), 0.5]], 'base', 2),
        line([[x(a), 0.5], [x(a + b * p), 0.5]], 'accent', 2)
      ]
    };
  }
};

/** The whole numbers from min to max, where random_int and round put their values */
const TICKS = [0.2, 0.35, 0.5, 0.65, 0.8];
const nearestTick = (v: number) =>
  TICKS.reduce((best, t) => (Math.abs(t - v) < Math.abs(best - v) ? t : best));
const ticks = (y: number, h: number): SketchShape[] =>
  TICKS.map((x) => line([[x, y - h], [x, y + h]], 'faint'));

const randomInt: NodeSketch = {
  input: 'min, max',
  output: 'integer',
  draw: (p, cycle) => ({
    input: [
      line([[0.1, 0.5], [0.9, 0.5]], 'faint'),
      ...ticks(0.5, 0.08),
      ...[0.2, 0.8].map((x) => line([[x, 0.32], [x, 0.68]], 'base', 2))
    ],
    output: [
      ...ticks(0.5, 0.45),
      ...range(RANDOM_ROWS).map((i) =>
        dot(
          [lerp(0.5, TICKS[Math.floor(hash(cycle * 7 + i + 50) * TICKS.length)], p), rowY(i)],
          'accent',
          2.5
        )
      )
    ]
  })
};

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

const VEC = [0.7, 0.55, 0.4];
const ORIGIN: SketchPoint = [0.4, 0.3];
const AXES: SketchPoint[] = [[0.5, -0.18], [0, 0.6], [-0.3, -0.18]];

const vec3: NodeSketch = {
  input: 'x, y, z',
  output: 'vec3',
  draw: (p) => {
    const tip = AXES.reduce<SketchPoint>(
      ([x, y], [ax, ay], i) => [x + ax * VEC[i] * p, y + ay * VEC[i] * p],
      ORIGIN
    );
    return {
      input: VEC.flatMap((v, i) => numberLine(v, 'base', 0.78 - i * 0.28)),
      output: [
        ...AXES.map(([ax, ay]) =>
          line([ORIGIN, [ORIGIN[0] + ax * 0.9, ORIGIN[1] + ay * 0.9]], 'faint')
        ),
        line([ORIGIN, tip], 'accent', 2),
        dot(tip, 'accent', 2.5)
      ]
    };
  }
};

const time: NodeSketch = {
  input: 'time.t',
  output: 'float',
  draw: (p) => {
    const x = lerp(0.1, 0.9, p);
    return {
      input: [
        line([[0.1, 0.5], [0.9, 0.5]], 'faint'),
        ...range(5).map((i) =>
          line([[lerp(0.1, 0.9, i / 4), 0.44], [lerp(0.1, 0.9, i / 4), 0.56]], 'faint')
        ),
        line([[x, 0.25], [x, 0.75]], 'base', 2)
      ],
      output: meter(p)
    };
  }
};

const mixColor: NodeSketch = {
  input: 'color, color',
  output: 'color',
  draw: (p) => ({
    input: [
      { kind: 'dot', at: [0.3, 0.5], tone: 'base', r: 10, mix: 0 },
      { kind: 'dot', at: [0.7, 0.5], tone: 'accent', r: 10, mix: 1 }
    ],
    output: [
      ...range(7).map((i): SketchShape => ({
        kind: 'dot',
        at: [lerp(0.2, 0.8, i / 6), 0.25],
        tone: 'base',
        r: 3,
        mix: i / 6
      })),
      line([[lerp(0.2, 0.8, p), 0.1], [lerp(0.2, 0.8, p), 0.16]], 'base'),
      { kind: 'dot', at: [0.5, 0.64], tone: 'base', r: 11, mix: p }
    ]
  })
};

export const CORE_SKETCHES: Record<string, NodeSketch> = {
  'max/core/curve': curve,
  'max/core/random': random,
  'max/core/alpha': alpha,
  'max/core/math': math,
  'max/core/random_int': randomInt,
  'max/core/round': round,
  'max/core/wave': wave,
  'max/core/vec3': vec3,
  'max/core/time': time,
  'max/core/mix_color': mixColor
};
