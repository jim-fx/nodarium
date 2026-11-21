import {
  area,
  ASPECT,
  band,
  circle,
  dot,
  hash,
  lerp,
  line,
  type NodeSketch,
  range,
  sample,
  type SketchPoint,
  type SketchShape,
  stem,
  stemAt,
  stemTangent,
  upright,
  upTo,
  walk
} from './kit';

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

/** A cap from the side: the stipe and its rays, each ray turned around the stipe and seen flat */
const TOP: SketchPoint = [0.5, 0.6];
const RAYS = 10;
const stipe = (tone: 'base' | 'faint' = 'base') => line(upright(0.5, 0.06, TOP[1]), tone, 3);

type CapShape = { open?: number; curl?: number; grown?: number };

function capRays({ open = 1, curl = 0, grown = 1 }: CapShape): SketchPoint[][] {
  const ray = walk([0, 0], 0.58, (t) => {
    const curled = Math.max(0, (t - 0.55) / 0.45);
    return 0.8 + 1.3 * t + (1 - open) * 1.4 * t + curl * 2.4 * curled * curled;
  }, 20);
  return range(RAYS).map((i) => {
    const turn = Math.cos((i / RAYS) * Math.PI * 2 + 0.3);
    return upTo(ray, grown).map(([x, y]): SketchPoint => [TOP[0] + x * turn, TOP[1] + y]);
  });
}

const cap = (shape: CapShape, tone: 'base' | 'accent' | 'faint' = 'base') =>
  capRays(shape).map((ray) => line(ray, tone));

/** A cap node that changes the rays, over the faint rays it got */
function reshapeCap(shape: (p: number) => CapShape): NodeSketch {
  return {
    input: 'path',
    output: 'path',
    draw: (p) => ({
      input: [stipe(), ...cap(shape(0))],
      output: [stipe(), ...cap(shape(0), 'faint'), ...cap(shape(p), 'accent')]
    })
  };
}

const capNode: NodeSketch = {
  input: 'path, points',
  output: 'path',
  draw: (p) => ({
    input: [stipe(), dot(TOP, 'base', 3)],
    output: [stipe(), ...cap({ grown: p * 0.98 + 0.02 }, 'accent')]
  })
};

const capOpen = reshapeCap((p) => ({ open: 1 - p }));
const marginCurl = reshapeCap((p) => ({ curl: p }));

/** The margin of a cap seen from a little above, a ring around the stipe */
const MARGIN: SketchPoint = [0.5, 0.48];
const marginAt = (a: number, lift = 0): SketchPoint => [
  MARGIN[0] + Math.cos(a) * 0.4,
  MARGIN[1] + Math.sin(a) * 0.13 + lift
];
const WAVES = 5;
const raysTo = (margin: (a: number) => SketchPoint, tone: 'base' | 'faint') =>
  range(12).map((i) => line([[0.5, 0.72], margin((i / 12) * Math.PI * 2)], tone));

const marginWaves: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const wavy = (a: number) => marginAt(a, 0.05 * p * Math.sin(a * WAVES));
    return {
      input: [
        stipe(),
        ...raysTo(marginAt, 'base'),
        line(sample(48, (t) => marginAt(t * Math.PI * 2)))
      ],
      output: [
        stipe('faint'),
        ...raysTo(wavy, 'faint'),
        line(sample(48, (t) => wavy(t * Math.PI * 2)), 'accent', 2)
      ]
    };
  }
};

const capMargin: NodeSketch = {
  input: 'path',
  output: 'points',
  draw: (p) => ({
    input: [stipe(), ...raysTo(marginAt, 'base')],
    output: [
      stipe('faint'),
      ...raysTo(marginAt, 'faint'),
      ...range(12)
        .filter((i) => p * 12 > i)
        .map((i) => dot(marginAt((i / 12) * Math.PI * 2), 'accent', 2.5))
    ]
  })
};

/** The top of a cap from the side, from margin to margin, and its underside */
const DOME = { left: 0.12, right: 0.88, base: 0.5, height: 0.36 };
const domeAt = (t: number): SketchPoint => [
  lerp(DOME.left, DOME.right, t),
  DOME.base + DOME.height * Math.sqrt(Math.max(0, 1 - (2 * t - 1) ** 2))
];
const undersideAt = (t: number): SketchPoint => [
  lerp(DOME.left, DOME.right, t),
  DOME.base + 0.1 * Math.sqrt(Math.max(0, 1 - (2 * t - 1) ** 2))
];
const domeArea = (tone: 'base' | 'faint' = 'base') =>
  area([...sample(24, domeAt), ...sample(24, undersideAt).reverse()], tone);
const capBody = (): SketchShape[] => [domeArea(), line(upright(0.5, 0.06, 0.58), 'base', 3)];

/** Where the gills, folds and tubes hang under the cap, the stipe in the middle is left free */
const GILLS = range(15).map((i) => (i + 0.5) / 15).filter((t) => Math.abs(t - 0.5) > 0.06);

const lamellae: NodeSketch = {
  input: 'path, pattern',
  output: 'geometry',
  draw: (p) => ({
    input: capBody(),
    output: [
      ...capBody(),
      ...GILLS.map((t) => {
        const [x, y] = undersideAt(t);
        const depth = 0.2 * Math.sqrt(Math.max(0, 1 - (2 * t - 1) ** 2));
        return line([[x, y], [x, y - depth * p]], 'accent');
      })
    ]
  })
};

const ridges: NodeSketch = {
  input: 'path, pattern',
  output: 'geometry',
  draw: (p) => ({
    input: capBody(),
    output: [
      ...capBody(),
      ...GILLS.filter((_, i) => i % 2 === 0).map((t) => {
        const from = undersideAt(t);
        const to: SketchPoint = [0.5 + (t < 0.5 ? -0.02 : 0.02), 0.24];
        return line(upTo([from, [lerp(from[0], to[0], 0.6), from[1] - 0.06], to], p), 'accent', 2);
      })
    ]
  })
};

const pores: NodeSketch = {
  input: 'path, pattern',
  output: 'geometry',
  draw: (p) => {
    const depth = 0.14 * p + 0.005;
    const bottom = (t: number): SketchPoint => {
      const [x, y] = undersideAt(t);
      return [x, y - depth * Math.sqrt(Math.max(0, 1 - (2 * t - 1) ** 2))];
    };
    return {
      input: capBody(),
      output: [
        ...capBody(),
        area([...sample(24, undersideAt), ...sample(24, bottom).reverse()], 'accent'),
        ...GILLS.map((t) => dot(bottom(t), 'base', 1.2))
      ]
    };
  }
};

/** The places of the marks of cap_pattern on the top of the cap */
const MARKS = range(9).map((i) => 0.1 + (i / 8) * 0.8);

const capPattern: NodeSketch = {
  input: 'geometry, pattern',
  output: 'geometry',
  draw: (p) => ({
    input: [domeArea()],
    output: [
      domeArea(),
      ...MARKS.map((t, i) => {
        const a = domeAt(t - 0.03 * p);
        const b = domeAt(t + 0.03 * p);
        const sink = 0.03 + 0.02 * hash(i);
        return line([[a[0], a[1] - sink], [b[0], b[1] - sink]], 'accent', 2);
      })
    ]
  })
};

/** A disc that a pattern node draws on */
const DISC: SketchPoint = [0.5, 0.5];
const DISC_R = 0.44;
const discPoint = (a: number, r: number): SketchPoint => [
  DISC[0] + Math.cos(a) * r * ASPECT,
  DISC[1] + Math.sin(a) * r
];
const discRim = line(circle(DISC, DISC_R), 'faint');

/** The patches of the cracks: a jittered grid, the cells inside the disc */
const CELLS = (() => {
  const n = 5;
  const corner = (i: number, j: number): SketchPoint => {
    const edge = i === 0 || j === 0 || i === n || j === n;
    const jitter = edge ? 0 : 0.35;
    return [
      lerp(-1, 1, (i + (hash(i * 13 + j) - 0.5) * jitter) / n),
      lerp(-1, 1, (j + (hash(i * 7 + j * 31) - 0.5) * jitter) / n)
    ];
  };
  return range(n * n)
    .map((k) => {
      const [i, j] = [k % n, Math.floor(k / n)];
      return [corner(i, j), corner(i + 1, j), corner(i + 1, j + 1), corner(i, j + 1)];
    })
    .filter((cell) => {
      const cx = cell.reduce((s, [x]) => s + x, 0) / 4;
      const cy = cell.reduce((s, [, y]) => s + y, 0) / 4;
      return Math.hypot(cx, cy) < 0.75;
    });
})();

const cracks: NodeSketch = {
  output: 'pattern',
  draw: (p) => ({
    input: [],
    output: [
      discRim,
      ...CELLS.map((cell) => {
        const cx = cell.reduce((s, [x]) => s + x, 0) / 4;
        const cy = cell.reduce((s, [, y]) => s + y, 0) / 4;
        const shrink = 1 - 0.3 * p;
        return area(
          [...cell, cell[0]].map(([x, y]): SketchPoint => [
            DISC[0] + (cx + (x - cx) * shrink) * DISC_R * ASPECT,
            DISC[1] + (cy + (y - cy) * shrink) * DISC_R
          ]),
          'base'
        );
      })
    ]
  })
};

/** Stripes that start further out each time the circumference has grown */
const STRIPES = [
  ...range(8).map((i) => ({ angle: (i / 8) * Math.PI * 2, start: 0.06 })),
  ...range(8).map((i) => ({ angle: ((i + 0.5) / 8) * Math.PI * 2, start: 0.2 })),
  ...range(16).map((i) => ({ angle: ((i + 0.5) / 16) * Math.PI * 2, start: 0.32 }))
];

const reactionDiffusion: NodeSketch = {
  output: 'pattern',
  draw: (p) => {
    const reach = lerp(0.08, DISC_R, p);
    return {
      input: [],
      output: [
        discRim,
        ...STRIPES.filter(({ start }) => start < reach).map(({ angle, start }) =>
          line([discPoint(angle, start), discPoint(angle, reach)], 'accent')
        )
      ]
    };
  }
};

/** A bundle of strands, the paths hyphae_to_mesh and set_fill give a body */
const BUNDLE = range(5).map((i) =>
  sample(
    24,
    (
      t
    ): SketchPoint => [
      0.5 + (i - 2) * 0.05 + 0.012 * Math.sin(t * 7 + i * 1.3),
      lerp(0.08, 0.92, t)
    ]
  )
);
const bundleLines = (tone: 'base' | 'faint') => BUNDLE.map((points) => line(points, tone));
const BODY = 0.3;

const hyphaeToMesh: NodeSketch = {
  input: 'path',
  output: 'geometry',
  draw: (p) => ({
    input: bundleLines('base'),
    output: [area(band(upright(), () => 0.02 + BODY * p), 'base'), ...bundleLines('faint')]
  })
};

const setFill: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const tube = [-1, 1].map((side) => line(upright(0.5 + (side * BODY) / 2 * ASPECT), 'faint'));
    return {
      input: [...tube, ...bundleLines('base')],
      output: [
        ...tube,
        area(band(upright(), () => 0.02 + (BODY - 0.02) * p), 'accent'),
        ...bundleLines('base')
      ]
    };
  }
};

/** Where the fibres of set_fibres run along the surface */
const FIBRES = range(14).map((i) => ({
  x: lerp(-0.4, 0.4, hash(i + 70)),
  y: lerp(0.12, 0.78, hash(i + 90))
}));

const setFibres: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const body = area(band(upright(), () => BODY), 'base');
    return {
      input: [body],
      output: [
        body,
        ...FIBRES.map(({ x, y }) => {
          const fx = 0.5 + x * BODY * ASPECT;
          return line([[fx, y], [fx, y + 0.02 + 0.12 * p]], 'accent');
        })
      ]
    };
  }
};

/** A veil from the margin of the cap to the stipe, sagging between them */
const MARGIN_SIDE: SketchPoint[] = [[0.18, 0.5], [0.82, 0.5]];
const STIPE_SIDE: SketchPoint[] = [[0.47, 0.28], [0.53, 0.28]];

const membrane: NodeSketch = {
  input: 'points, points',
  output: 'geometry',
  draw: (p) => {
    const anchors = [...MARGIN_SIDE, ...STIPE_SIDE].map((at) => dot(at, 'base', 2.5));
    const skin = (k: number) =>
      sample(12, (t): SketchPoint => {
        const [a, b] = [MARGIN_SIDE[k], STIPE_SIDE[k]];
        return [lerp(a[0], b[0], t), lerp(a[1], b[1], t) - 0.1 * Math.sin(Math.PI * t)];
      });
    return {
      input: [...capBody(), ...anchors],
      output: [...capBody(), ...[0, 1].map((k) => line(upTo(skin(k), p), 'accent', 2)), ...anchors]
    };
  }
};

/** The ring on the stipe that a skirt hangs from */
const RING: SketchPoint = [0.5, 0.72];

const skirt: NodeSketch = {
  input: 'points',
  output: 'geometry',
  draw: (p) => {
    const ring = range(10).map((i) => {
      const a = (i / 10) * Math.PI * 2;
      return dot([RING[0] + Math.cos(a) * 0.1, RING[1] + Math.sin(a) * 0.03], 'base', 2);
    });
    const length = 0.04 + 0.4 * p;
    const top = sample(
      16,
      (
        t
      ): SketchPoint => [
        RING[0] - Math.cos(t * Math.PI) * 0.1,
        RING[1] - Math.sin(t * Math.PI) * 0.03
      ]
    );
    const edge = sample(16, (t): SketchPoint => {
      const flare = 0.1 + 0.08 * p;
      const torn = 0.05 * p * hash(Math.round(t * 16) + 5);
      return [RING[0] - Math.cos(t * Math.PI) * flare, RING[1] - length + torn];
    });
    return {
      input: [line(upright(0.5, 0.06, 0.94), 'base', 3), ...ring],
      output: [
        line(upright(0.5, 0.06, 0.94), 'base', 3),
        area([...top, ...edge.reverse()], 'accent'),
        ...ring
      ]
    };
  }
};

/** A mound of forest floor, the surface sample_mesh spreads points over */
const moundAt = (x: number) => 0.12 + 0.4 * Math.exp(-(((x - 0.5) / 0.22) ** 2));
const mound = area([
  ...sample(32, (t): SketchPoint => [lerp(0.04, 0.96, t), moundAt(lerp(0.04, 0.96, t))]),
  [0.96, 0.05],
  [0.04, 0.05]
], 'base');
const SAMPLES = range(9).map((i) => lerp(0.12, 0.88, (i + hash(i + 400) * 0.6) / 9));

const sampleMesh: NodeSketch = {
  input: 'geometry',
  output: 'points',
  draw: (p) => ({
    input: [mound],
    output: [
      mound,
      ...SAMPLES.filter((_, i) => p * SAMPLES.length > i).flatMap((x) => {
        const y = moundAt(x);
        const slope = (moundAt(x + 0.01) - moundAt(x - 0.01)) / 0.02;
        const l = Math.hypot(slope, 1);
        const tip: SketchPoint = [x - (slope / l) * 0.08 * ASPECT, y + 0.08 / l];
        return [line([[x, y], tip], 'accent'), dot([x, y], 'accent', 2)];
      })
    ]
  })
};

export const MYZEL_SKETCHES: Record<string, NodeSketch> = {
  'max/myzel/strands': strands,
  'max/myzel/cap': capNode,
  'max/myzel/cap_open': capOpen,
  'max/myzel/margin_curl': marginCurl,
  'max/myzel/margin_waves': marginWaves,
  'max/myzel/cap_margin': capMargin,
  'max/myzel/lamellae': lamellae,
  'max/myzel/ridges': ridges,
  'max/myzel/pores': pores,
  'max/myzel/cap_pattern': capPattern,
  'max/myzel/cracks': cracks,
  'max/myzel/reaction_diffusion': reactionDiffusion,
  'max/myzel/hyphae_to_mesh': hyphaeToMesh,
  'max/myzel/set_fill': setFill,
  'max/myzel/set_fibres': setFibres,
  'max/myzel/membrane': membrane,
  'max/myzel/skirt': skirt,
  'max/myzel/sample_mesh': sampleMesh
};
