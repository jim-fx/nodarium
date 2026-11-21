import {
  along,
  area,
  ASPECT,
  band,
  circle,
  clamp,
  dot,
  ease,
  ellipse,
  leaf,
  lerp,
  line,
  meter,
  type NodeSketch,
  offset,
  range,
  sample,
  type SketchPoint,
  type SketchShape,
  type SketchTone,
  stem,
  stemAt,
  upright,
  upTo,
  walk
} from './kit';

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

const BASE: SketchPoint = [0.5, 0.08];
const STEM_LENGTH = 0.84;
const thick = (points: SketchPoint[], tone: SketchTone = 'base') => line(points, tone, 2);

/**
 * A node that bends or moves paths: the path as it comes in, and as it goes out over the faint
 * path it came in as
 */
function reshape(
  paths: (p: number) => SketchPoint[][],
  extra?: (p: number) => SketchShape[]
): NodeSketch {
  return {
    input: 'path',
    output: 'path',
    draw: (p) => ({
      input: [...paths(0).map((points) => thick(points)), ...(extra?.(0) ?? [])],
      output: [
        ...paths(0).map((points) => line(points, 'faint')),
        ...paths(p).map((points) => thick(points, 'accent')),
        ...(extra?.(p) ?? [])
      ]
    })
  };
}

const gravity = reshape((p) => [walk(BASE, STEM_LENGTH, (t) => 0.1 + p * 1.9 * t * t)]);

const noise = reshape((p) => [offset(upright(), (t) => p * 0.1 * t * Math.sin(t * 11 + 0.6))]);

const rotate = reshape(
  (p) => [walk(BASE, STEM_LENGTH, (t) => 0.2 * t - p * 0.7)],
  (
    p
  ) => [
    line(
      sample(
        8,
        (s) => [0.5 + Math.sin(-0.7 * p * s) * 0.3 * ASPECT, 0.08 + Math.cos(-0.7 * p * s) * 0.3]
      ),
      'faint'
    )
  ]
);

/** Three paths side by side, a bundle seen from the side */
const PHASES = [-Math.PI / 2, 0, Math.PI / 2];

const twist = reshape((p) =>
  PHASES.map((phase) =>
    sample(32, (t) => [0.5 + 0.12 * Math.sin(phase + p * Math.PI * 3 * t * t), lerp(0.08, 0.92, t)])
  )
);

const roll = reshape((p) => {
  const r = 0.55 * p;
  return [
    walk([0.42, 0.08], STEM_LENGTH, (t) => {
      if (r === 0 || t < 1 - r) return 0;
      const u = (t - (1 - r)) / r;
      return Math.PI * 2 * 2 * r * u * u;
    }, 64)
  ];
});

/** Paths that stand side by side at the bottom of the panel */
const row = (n: number, from = 0.2, to = 0.8) => range(n).map((i) => lerp(from, to, i / (n - 1)));

const clump = reshape((p) =>
  row(6).map((x, i) => {
    const center = i < 3 ? 0.36 : 0.64;
    return sample(16, (t) => [lerp(x, lerp(x, center, 0.85 * p), ease(t)), lerp(0.08, 0.92, t)]);
  })
);

const avoid = reshape((p) =>
  [-1, 1].map((side) =>
    walk(
      [0.5 + side * 0.14, 0.08],
      STEM_LENGTH,
      (t) => -side * (0.5 - p * 0.75 * ease(clamp(t * 1.6)))
    )
  )
);

const KEEP_APART = 0.11;

const keepApart = reshape(
  (p) => [-1, 0, 1].map((k) => walk([0.5 + k * 0.08, 0.08], 0.64, () => k * 0.55 * p)),
  (p) =>
    [-1, 0, 1].map((k) => {
      const tip = walk([0.5 + k * 0.08, 0.08], 0.64, () => k * 0.55 * p).at(-1)!;
      return line(circle([tip[0], tip[1] + 0.06], KEEP_APART, 24), 'faint');
    })
);

/** The bundles the strands of neighbour_sensing end up in */
const SENSED = [0.3, 0.3, 0.55, 0.55, 0.55, 0.75, 0.75];

const neighbourSensing = reshape((p) =>
  row(7).map((x, i) =>
    sample(32, (t) => [
      lerp(x, SENSED[i], 0.75 * p * ease(t)) + p * 0.025 * Math.sin(t * 13 + i * 2.1),
      lerp(0.08, 0.92, t)
    ])
  )
);

const pathInfo: NodeSketch = {
  input: 'path',
  output: 'float',
  draw: (p) => {
    const xs = row(4, 0.2, 0.68);
    return {
      input: xs.map((x, i) => thick(walk([x, 0.08], 0.5 + i * 0.1, () => 0.1))),
      output: xs.flatMap((x, i) => meter((i / 3) * p, x).slice(0, 2))
    };
  }
};

const separate: NodeSketch = {
  input: 'path, like',
  output: 'path, path',
  draw: (p) => {
    const paths = (shift: number) =>
      row(6, 0.3, 0.7).map((x, i) => {
        const liked = i % 2 === 1;
        const dx = (liked ? 0.14 : -0.14) * shift;
        return thick(
          walk([x + dx, 0.08], 0.7, () => (liked ? 0.1 : -0.1)),
          liked ? 'accent' : 'base'
        );
      });
    return { input: paths(0), output: paths(p) };
  }
};

const toGeometry: NodeSketch = {
  input: 'path',
  output: 'geometry',
  draw: (p) => {
    const path = walk(BASE, STEM_LENGTH, (t) => 0.3 * t);
    return {
      input: [thick(path)],
      output: [
        area(band(path, (t) => 0.02 + 0.16 * (1 - 0.7 * t) * p), 'base'),
        line(path, 'faint')
      ]
    };
  }
};

/** Spots along the stem where points_on_path puts a point, turned around the stem in turn */
const SPOTS = [0.25, 0.37, 0.49, 0.61, 0.73, 0.85];
const spot = (t: number) => along(upright(), t);
const shown = (i: number, n: number, p: number) => p * n > i;

const pointsOnPath: NodeSketch = {
  input: 'path',
  output: 'points',
  draw: (p) => ({
    input: [thick(upright())],
    output: [
      line(upright(), 'faint'),
      ...SPOTS.filter((_, i) => shown(i, SPOTS.length, p)).flatMap((t, i) => {
        const [x, y] = spot(t);
        const side = i % 2 ? 1 : -1;
        return [line([[x, y], [x + side * 0.1, y + 0.04]], 'accent'), dot([x, y], 'accent', 2.5)];
      })
    ]
  })
};

/** A stem with points on it, the way points_on_path gives them */
const pointsOn = (tone: SketchTone): SketchShape[] => [
  thick(upright()),
  ...SPOTS.filter((_, i) => i % 2 === 0 || i === 5).map((t) => dot(spot(t), tone, 2.5))
];
const POINT_SPOTS = [0.25, 0.49, 0.73, 0.85];

const branchFromPoints: NodeSketch = {
  input: 'path, points',
  output: 'path',
  draw: (p) => ({
    input: pointsOn('base'),
    output: [
      ...pointsOn('faint'),
      ...POINT_SPOTS.map((t, i) =>
        line(walk(spot(t), 0.32 * (1 - t * 0.5) * p, () => (i % 2 ? 0.9 : -0.9), 4), 'accent', 1.5)
      )
    ]
  })
};

const leaves = (spots: number[], p: number): SketchShape[] =>
  spots.map((t, i) => area(leaf(spot(t), i % 2 ? 1.1 : -1.1, 0.24 * p + 0.001), 'accent'));

const instance: NodeSketch = {
  input: 'path, geometry',
  output: 'path',
  draw: (p) => ({
    input: [thick(upright(0.4)), area(leaf([0.75, 0.55], 0.3, 0.3), 'base')],
    output: [thick(upright()), ...leaves(SPOTS.slice(0, 5), p)]
  })
};

const instanceFromPoints: NodeSketch = {
  input: 'points, geometry',
  output: 'path',
  draw: (p) => ({
    input: [
      line(upright(), 'faint'),
      ...pointsOn('base').slice(1),
      area(leaf([0.78, 0.6], 0.3, 0.25), 'base')
    ],
    output: [...pointsOn('faint'), ...leaves(POINT_SPOTS, p)]
  })
};

/** Where trunk_kink bends the path, and which way */
const KINKS: [number, number][] = [[0.3, 1], [0.55, -1], [0.8, 1]];

const kinked = (p: number) =>
  walk(
    [0.46, 0.08],
    STEM_LENGTH,
    (t) => KINKS.reduce((a, [at, side]) => (t > at ? a + side * 0.4 * p : a), 0),
    48
  );

const trunkKink: NodeSketch = {
  input: 'path',
  output: 'path, points',
  draw: (p) => {
    const path = kinked(p);
    return {
      input: [thick(kinked(0))],
      output: [
        line(kinked(0), 'faint'),
        thick(path, 'accent'),
        ...KINKS.map(([at]) => dot(along(path, at), 'accent', 2.5))
      ]
    };
  }
};

/** The paths that random_break breaks, and where */
const BREAKS: Record<number, number> = { 1: 0.45, 3: 0.6 };

const randomBreak: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const paths = row(5).map((x, i) => walk([x, 0.08], 0.78, () => (i - 2) * 0.06));
    return {
      input: paths.map((points) => thick(points)),
      output: paths.flatMap((points, i) => {
        const at = BREAKS[i];
        if (at === undefined) return [thick(points)];
        const lower = upTo(points, at);
        const upper = points.slice(Math.ceil(at * (points.length - 1)));
        return [
          thick(lower),
          line(upper.map(([x, y]) => [x + 0.08 * p, y - 0.12 * p]), 'faint'),
          dot(lower.at(-1)!, 'accent', 2.5)
        ];
      })
    };
  }
};

const HEIGHTS = [0.62, 0.84, 0.72, 0.9, 0.56];

const cut: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const height = lerp(0.94, 0.5, p);
    const paths = row(5).map((x, i) => walk([x, 0.06], HEIGHTS[i] - 0.06, () => (i - 2) * 0.05));
    return {
      input: paths.map((points) => thick(points)),
      output: [
        line([[0.06, height], [0.94, height]], 'faint'),
        ...paths.flatMap((points) => {
          const below = points.filter(([, y]) => y <= height);
          return [
            thick(below.length > 1 ? below : points.slice(0, 2), 'accent'),
            line(points, 'faint')
          ];
        })
      ]
    };
  }
};

/** A fan seen from above, the rays leave the centre and spread to the margin */
const CENTER: SketchPoint = [0.5, 0.5];
const rayPoint = (angle: number, r: number): SketchPoint => [
  CENTER[0] + Math.cos(angle) * r * ASPECT,
  CENTER[1] + Math.sin(angle) * r
];

const fillGaps: NodeSketch = {
  input: 'path',
  output: 'path',
  draw: (p) => {
    const rays = range(7).map((i) =>
      line([rayPoint((i / 7) * Math.PI * 2, 0.06), rayPoint((i / 7) * Math.PI * 2, 0.44)])
    );
    return {
      input: rays,
      output: [
        ...rays,
        ...range(7).map((i) => {
          const a = ((i + 0.5) / 7) * Math.PI * 2;
          return line([rayPoint(a, 0.2), rayPoint(a, lerp(0.2, 0.44, p))], 'accent');
        })
      ]
    };
  }
};

const spiral: NodeSketch = {
  input: 'points',
  output: 'points',
  draw: (p) => ({
    input: [dot(CENTER, 'base', 3)],
    output: [
      dot(CENTER, 'base', 2),
      ...range(9)
        .filter((i) => shown(i, 9, p))
        .flatMap((i) => {
          const end = rayPoint(i * 2.3998, 0.14 + i * 0.03);
          return [line([CENTER, end], 'faint'), dot(end, 'accent', 2.5)];
        })
    ]
  })
};

const umbel: NodeSketch = {
  input: 'path',
  output: 'path, points',
  draw: (p) => {
    const path = upright(0.5, 0.08, 0.56);
    const tip = path.at(-1)!;
    return {
      input: [thick(path)],
      output: [
        thick(path),
        ...range(7).flatMap((i) => {
          const end = walk(tip, 0.36 * p, () => lerp(-0.95, 0.95, i / 6), 2).at(-1)!;
          return [line([tip, end], 'accent'), dot(end, 'accent', 2)];
        })
      ]
    };
  }
};

const FORCE: SketchPoint = [0.8, 0.7];

const force: NodeSketch = {
  output: 'force',
  draw: (p) => ({
    input: [],
    output: [
      line(circle(CENTER, 0.42), 'faint'),
      dot(CENTER, 'base', 3),
      ...range(8).map((i) => {
        const from = rayPoint((i / 8) * Math.PI * 2, 0.36);
        return line([from, along([from, CENTER], 0.05 + 0.5 * p)], 'accent');
      })
    ]
  })
};

const applyForce: NodeSketch = {
  input: 'path, force',
  output: 'path',
  draw: (p) => {
    const paths = (q: number) =>
      row(3, 0.25, 0.55).map((x) => walk([x, 0.08], 0.78, (t) => q * 1.1 * Math.pow(t, 1.5)));
    const pull = [dot(FORCE, 'base', 3), line(circle(FORCE, 0.2), 'faint')];
    return {
      input: [...paths(0).map((points) => thick(points)), ...pull],
      output: [
        ...paths(0).map((points) => line(points, 'faint')),
        ...paths(p).map((points) => thick(points, 'accent')),
        ...pull
      ]
    };
  }
};

/** One half of a leaf from its base to its tip, the way the shape node draws it */
const halfLeaf = (width: number, bulge = 0.8): SketchPoint[] =>
  sample(20, (t) => [0.5 + width * Math.sin(Math.PI * Math.pow(t, bulge)), lerp(0.08, 0.92, t)]);
const mirrored = (points: SketchPoint[], s = 1): SketchPoint[] =>
  points.map(([x, y]) => [0.5 - (x - 0.5) * s, y]);
const midrib = line(upright(), 'faint');

const leafNode: NodeSketch = {
  input: 'shape',
  output: 'geometry',
  draw: (p) => {
    const half = halfLeaf(0.3);
    return {
      input: [midrib, thick(half)],
      output: [
        area([...half, ...mirrored(half, p).reverse()], 'base'),
        midrib,
        line(half),
        line(mirrored(half, p), 'accent')
      ]
    };
  }
};

const VEINS = [0.3, 0.5, 0.7];
const veins = (): SketchPoint[][] => [
  upright(),
  ...VEINS.flatMap((t) =>
    [-1, 1].map((side) => walk(spot(t), 0.3 * (1 - t * 0.4), () => side * 0.9, 4))
  )
];

const leafSurface: NodeSketch = {
  input: 'path',
  output: 'geometry',
  draw: (p) => ({
    input: veins().map((v) => thick(v)),
    output: [
      area(
        band(upright(), (t) => (0.04 + 0.58 * Math.sin(Math.PI * Math.pow(t, 0.8))) * p),
        'base'
      ),
      ...veins().map((v) => line(v))
    ]
  })
};

const mixShape: NodeSketch = {
  input: 'shape, shape',
  output: 'shape',
  draw: (p) => {
    const [a, b] = [halfLeaf(0.16, 0.5), halfLeaf(0.36, 1.2)];
    const mixed = a.map(([x, y], i): SketchPoint => [lerp(x, b[i][0], p), y]);
    return {
      input: [midrib, thick(a), line(b, 'faint')],
      output: [midrib, line(a, 'faint'), line(b, 'faint'), thick(mixed, 'accent')]
    };
  }
};

/** The profile a revolve turns around its midrib, a bud */
const profile = sample(
  20,
  (t): SketchPoint => [0.5 + 0.3 * Math.sin(Math.PI * t) * (1 - 0.4 * t), lerp(0.1, 0.9, t)]
);

const revolve: NodeSketch = {
  input: 'shape',
  output: 'geometry',
  draw: (p) => ({
    input: [midrib, thick(profile)],
    output: [
      area([...profile, ...mirrored(profile, p).reverse()], 'base'),
      line(profile),
      line(mirrored(profile, p), 'accent'),
      ...[0.3, 0.5, 0.7].map((t) => {
        const [x, y] = along(profile, t);
        return line(ellipse([0.5, y], (x - 0.5) * p + 0.001, 0.04 * p + 0.001, 24), 'faint');
      })
    ]
  })
};

/** A leaf seen from its side, growing to the right from its base */
const sideLeaf = (curl: number) => {
  const rib = walk([0.12, 0.3], 1.05, (t) => Math.PI / 2 - curl * 2.2 * Math.pow(t, 1.5), 24);
  return area(band(rib, (t) => 0.09 * Math.sin(Math.PI * Math.pow(t, 0.7)) + 0.01), 'base');
};

const deform: NodeSketch = {
  input: 'geometry',
  output: 'geometry',
  draw: (p) => ({
    input: [sideLeaf(0)],
    output: [{ ...sideLeaf(0), tone: 'faint' }, { ...sideLeaf(p), tone: 'accent' }]
  })
};

export const PLANTARIUM_SKETCHES: Record<string, NodeSketch> = {
  'max/plantarium/branch': branch,
  'max/plantarium/gravity': gravity,
  'max/plantarium/noise': noise,
  'max/plantarium/rotate': rotate,
  'max/plantarium/twist': twist,
  'max/plantarium/roll': roll,
  'max/plantarium/clump': clump,
  'max/plantarium/avoid': avoid,
  'max/plantarium/keep_apart': keepApart,
  'max/plantarium/neighbour_sensing': neighbourSensing,
  'max/plantarium/path_info': pathInfo,
  'max/plantarium/separate': separate,
  'max/plantarium/to_geometry': toGeometry,
  'max/plantarium/points_on_path': pointsOnPath,
  'max/plantarium/branch_from_points': branchFromPoints,
  'max/plantarium/instance': instance,
  'max/plantarium/instance_from_points': instanceFromPoints,
  'max/plantarium/trunk_kink': trunkKink,
  'max/plantarium/random_break': randomBreak,
  'max/plantarium/cut': cut,
  'max/plantarium/fill_gaps': fillGaps,
  'max/plantarium/spiral': spiral,
  'max/plantarium/umbel': umbel,
  'max/plantarium/force': force,
  'max/plantarium/apply_force': applyForce,
  'max/plantarium/leaf': leafNode,
  'max/plantarium/leaf_surface': leafSurface,
  'max/plantarium/mix_shape': mixShape,
  'max/plantarium/revolve': revolve,
  'max/plantarium/deform': deform
};
