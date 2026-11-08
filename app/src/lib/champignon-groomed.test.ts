// The groomed Champignon template matures with time: every stage from the button (t = 0) to the
// open cap (t = 1) is checked in steps of 0.05 with the compiled nodes, as the research in #175
// asks. It runs the guides only (stipe, cap rays, veil, gills), not the mesh.
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { splitNestedArray } from '@nodarium/utils';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { BenchmarkRegistry } from '../../benchmark/benchmarkRegistry';
import template from './graph-templates/champignon-groomed.json';
import { expandGroups, MemoryRuntimeExecutor } from './runtime/runtime-executor';
import { MemoryRuntimeCache } from './runtime/runtime-executor-cache';

type Vec = [number, number, number];
type Point = { p: Vec; r: number };
type Path = { id: number; parent: number; alpha: number; points: Point[] };

function readPaths(data: Int32Array): Path[] {
  return splitNestedArray(data)
    .filter((item) => item[0] === 0)
    .map((item) => {
      const floats = new Float32Array(item.buffer, item.byteOffset + 4 * 4, item.length - 4);
      const points: Point[] = [];
      for (let i = 3; i + 3 < floats.length; i += 4) {
        points.push({ p: [floats[i], floats[i + 1], floats[i + 2]], r: Math.abs(floats[i + 3]) });
      }
      return { id: item[2], parent: item[3], alpha: floats[0], points };
    });
}

const sub = (a: Vec, b: Vec): Vec => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: Vec, b: Vec) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const length = (a: Vec) => Math.sqrt(dot(a, a));

/** Whether two segments in a plane cross, touching at an end doesn't count */
function cross(a: number[], b: number[], c: number[], d: number[]) {
  const side = (p: number[], q: number[], r: number[]) =>
    (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
  const [d1, d2, d3, d4] = [side(c, d, a), side(c, d, b), side(a, b, c), side(a, b, d)];
  return d1 * d2 < 0 && d3 * d4 < 0;
}

/** Whether a polyline in a plane crosses itself */
function folds(line: number[][]) {
  for (let i = 0; i + 1 < line.length; i++) {
    for (let j = i + 2; j + 1 < line.length; j++) {
      if (cross(line[i], line[i + 1], line[j], line[j + 1])) return true;
    }
  }
  return false;
}

/**
 * The gills of lamellae as plates: every gill is a stack of paths with the same samples, from the
 * top in the flesh down to the edge, one after the other
 */
function plates(paths: Path[]): Path[][] {
  const gills: Path[][] = [];
  for (const path of paths) {
    const gill = gills[gills.length - 1];
    const previous = gill?.[gill.length - 1];
    const stacked = previous && previous.points.length === path.points.length
      && path.points.every((q, m) => length(sub(q.p, previous.points[m].p)) < 5e-4);
    if (stacked) gill.push(path);
    else gills.push([path]);
  }
  return gills;
}

/** A Myzel mesh `[14, v, f, flags, ...faces, ...positions, ...]`: its faces and positions */
type Mesh = { faces: Int32Array; positions: Float32Array };

function readMesh(data: Int32Array): Mesh {
  const [v, f] = [data[1], data[2]];
  return {
    faces: data.subarray(4, 4 + 3 * f),
    positions: new Float32Array(data.buffer, data.byteOffset + (4 + 3 * f) * 4, 3 * v)
  };
}

/** Every edge is walked once each way: the mesh is closed and its faces are wound alike */
function isClosed(mesh: Mesh) {
  const edges = new Map<string, number>();
  const { faces } = mesh;
  for (let i = 0; i < faces.length; i += 3) {
    for (const [a, b] of [[0, 1], [1, 2], [2, 0]]) {
      const key = `${faces[i + a]} ${faces[i + b]}`;
      edges.set(key, (edges.get(key) ?? 0) + 1);
    }
  }
  return [...edges].every(([key, n]) => n === 1 && edges.has(key.split(' ').reverse().join(' ')));
}

type Stage = {
  t: number;
  stipe: Path;
  rays: Path[];
  capRays: Path[];
  veil: Path[];
  skin: Mesh;
  gills: Path[];
  /** The stipe top and its direction, the cap's axis */
  hinge: Vec;
  axis: Vec;
};

/** Where a point sits in the plane of a ray: distance from the cap's axis and height over the hinge */
function plane(s: Stage, p: Vec): [number, number] {
  const q = sub(p, s.hinge);
  const along = dot(q, s.axis);
  return [length(sub(q, [s.axis[0] * along, s.axis[1] * along, s.axis[2] * along])), along];
}

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
// CI builds the nodes before the unit tests, a local run without `pnpm build:nodes` skips this
const BUILT = existsSync(resolve('static/nodes/max/myzel/membrane.wasm'));
const STEPS = Array.from({ length: 21 }, (_, i) => Math.round(i * 5) / 100);

describe.skipIf(!BUILT && !ON_CI)('the groomed Champignon matures with time', () => {
  const stages: Stage[] = [];

  beforeAll(async () => {
    const registry = new BenchmarkRegistry();
    // the reaction-diffusion pattern doesn't depend on time, the cache keeps it between stages
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const base = expandGroups(structuredClone(template) as unknown as Graph);
    await registry.load(
      base.nodes.map((n) => n.type).filter((t) => !t.startsWith('__')) as NodeId[]
    );
    const output = base.nodes.find((n) => n.type === 'max/myzel/output')!;
    const of = (type: string) => base.nodes.find((n) => n.type === type)!.id;
    const cap = of('max/myzel/cap');
    const probes = {
      // the stipe as the cap grows on it
      stipe: [base.edges.find((e) => e[2] === cap && e[3] === 'plant')![0], 0],
      rays: [of('max/myzel/cap_open'), 0],
      capRays: [cap, 0],
      veil: [of('max/myzel/membrane'), 0],
      gills: [of('max/myzel/lamellae'), 0]
    } as const;
    // a node shows one output to the debug nodes, the membrane's skin needs a run of its own
    const skin = { skin: [of('max/myzel/membrane'), 1] } as const;
    const run = async (t: number, debugged: Record<string, readonly [number, number]>) => {
      const graph = structuredClone(base);
      // nothing goes to the output, only what the debug nodes read runs, not the mesh
      graph.edges = graph.edges.filter((e) => e[2] !== output.id);
      let debugId = 1e9;
      for (const [id, out] of Object.values(debugged)) {
        graph.nodes.push({ id: ++debugId, type: DEBUG_NODE as NodeId, position: [0, 0] });
        graph.edges.push([id, out, debugId, 'input']);
      }
      await runtime.execute(graph, { ...graph.settings, 'time.t': t, seed: 1 });
      return runtime.getDebugData();
    };

    for (const t of STEPS) {
      const debug = await run(t, probes);
      const read = (key: keyof typeof probes) => readPaths(debug[probes[key][0]].data);
      const stipe = read('stipe')[0];
      const [top, below] = [stipe.points.at(-1)!.p, stipe.points.at(-2)!.p];
      const direction = sub(top, below);
      const capRays = read('capRays');
      stages.push({
        t,
        stipe,
        rays: read('rays'),
        // the rays the cap node grows run from the axis, fill_gaps adds more further out
        capRays: read('rays').filter((r) => capRays.some((c) => c.id === r.id)),
        veil: read('veil'),
        skin: readMesh((await run(t, skin))[skin.skin[0]].data),
        gills: read('gills'),
        hinge: top,
        axis: direction.map((v) => v / length(direction)) as Vec
      });
    }
  }, 120_000);

  const at = (t: number) => stages.find((s) => Math.abs(s.t - t) < 1e-6)!;
  const mean = (values: number[]) => values.reduce((a, b) => a + b, 0) / values.length;
  const capDiameter = (s: Stage) =>
    2 * Math.max(...s.rays.flatMap((r) => r.points.map((q) => plane(s, q.p)[0] + q.r)));
  const stipeLength = (s: Stage) =>
    s.stipe.points.slice(1).reduce((sum, q, i) => sum + length(sub(q.p, s.stipe.points[i].p)), 0);
  const ring = (s: Stage) => s.veil.filter((v) => v.parent === s.stipe.id);
  const fringe = (s: Stage) => s.veil.filter((v) => v.parent !== s.stipe.id);

  it('is at real size, a button 25-32 mm wide that opens to 50-60 mm', () => {
    expect(capDiameter(at(0))).toBeGreaterThan(0.025);
    expect(capDiameter(at(0))).toBeLessThan(0.032);
    expect(capDiameter(at(1))).toBeGreaterThan(0.05);
    expect(capDiameter(at(1))).toBeLessThan(0.06);
    // the stipe is 3-6 cm tall and 1-2 cm thick when open
    expect(stipeLength(at(1))).toBeGreaterThan(0.03);
    expect(stipeLength(at(1))).toBeLessThan(0.06);
    const thickness = 2 * Math.max(...at(1).stipe.points.map((q) => q.r));
    expect(thickness).toBeGreaterThan(0.01);
    expect(thickness).toBeLessThan(0.02);
  });

  it('grows the cap and stipe at every step and never jumps', () => {
    const margin = (s: Stage) => {
      const ends = s.capRays.map((r) => plane(s, r.points.at(-1)!.p));
      return [mean(ends.map((e) => e[0])), mean(ends.map((e) => e[1]))];
    };
    for (let i = 1; i < stages.length; i++) {
      const [a, b] = [stages[i - 1], stages[i]];
      expect(capDiameter(b), `t = ${b.t}`).toBeGreaterThanOrEqual(capDiameter(a) - 1e-5);
      expect(stipeLength(b), `t = ${b.t}`).toBeGreaterThanOrEqual(stipeLength(a) - 1e-5);
      // the margin moves less than 3 mm from one step to the next
      const [ma, mb] = [margin(a), margin(b)];
      expect(Math.hypot(mb[0] - ma[0], mb[1] - ma[1]), `t = ${b.t}`).toBeLessThan(0.003);
    }
  });

  it('never folds the cap profile or crosses the axis (Desbenoit et al. 2004)', () => {
    for (const s of stages) {
      for (const ray of s.rays) {
        // every ray lies in a plane through the axis
        const line = ray.points.map((q) => plane(s, q.p));
        expect(folds(line), `ray ${ray.id} at t = ${s.t}`).toBe(false);
      }
      // a closed margin lies against the stipe, never inside it
      const stipeRadius = s.stipe.points.at(-1)!.r;
      for (const ray of s.capRays) {
        expect(plane(s, ray.points.at(-1)!.p)[0]).toBeGreaterThan(stipeRadius);
      }
    }
  });

  it('never folds the gills, they stay under the cap and hang deeper as it opens', () => {
    let deepest = 0;
    const depths: number[] = [];
    for (const s of stages) {
      const reach = Math.max(...s.rays.flatMap((r) => r.points.map((q) => plane(s, q.p)[0])));
      const top = Math.max(...s.rays.flatMap((r) => r.points.map((q) => plane(s, q.p)[1])));
      let depth = 0;
      for (const gill of plates(s.gills)) {
        // the plate runs from its top in the flesh down to its edge, the way from one to the
        // other turns smoothly along the gill and never flips: the plate doesn't fold
        const [first, last] = [gill[0].points, gill.at(-1)!.points];
        const down = first.map((q, m) => sub(last[m].p, q.p));
        for (let m = 1; m < down.length; m++) {
          expect(dot(down[m - 1], down[m]), `gill ${gill[0].id} at t = ${s.t}`).toBeGreaterThan(0);
        }
        depth = Math.max(depth, ...down.map(length));
        for (const path of gill) {
          const points = path.points.map((q) => plane(s, q.p));
          expect(Math.max(...points.map((q) => q[0]))).toBeLessThan(reach);
          expect(Math.max(...points.map((q) => q[1]))).toBeLessThan(top);
        }
      }
      // the gills get deeper as the cap matures and never shrink back. The plates reach into the
      // flesh by a share of its thickness, which thins a little while the cap spreads (< 0.3 mm)
      expect(depth, `t = ${s.t}`).toBeGreaterThanOrEqual(deepest - 3e-4);
      deepest = Math.max(deepest, depth);
      depths.push(depth);
    }
    // from about 0.5 mm on the button to about 5 mm when open
    expect(depths.at(-1)! - depths[0]).toBeGreaterThan(0.004);
  });

  it('keeps the veil intact on the button and tears it into a ring and a fringe', () => {
    for (const s of stages.filter((s) => s.t <= 0.3)) {
      expect(ring(s).length, `t = ${s.t}`).toBeGreaterThan(0);
      expect(fringe(s), `t = ${s.t}`).toEqual([]);
    }
    // open, every strand has torn: no part on the stipe reaches out to the margin any more, and
    // the margin keeps flecks with gaps between them
    const open = at(1);
    const margin = capDiameter(open) / 2;
    for (const part of ring(open)) {
      expect(plane(open, part.points.at(-1)!.p)[0]).toBeLessThan(margin - 0.01);
    }
    expect(fringe(open).length).toBeGreaterThan(ring(open).length / 4);
    expect(fringe(open).length).toBeLessThanOrEqual(ring(open).length);
  });

  it('keeps the ring in the upper third while the stipe grows mainly above it', () => {
    // how far up the stipe the ring is, along it
    const height = (s: Stage) => mean(ring(s).map((v) => v.alpha)) * stipeLength(s);
    const open = at(1);
    // in the upper third when open, below the cap, as in photos of A. bisporus
    expect(height(open) / stipeLength(open)).toBeGreaterThan(2 / 3);
    expect(height(open) / stipeLength(open)).toBeLessThan(0.85);
    // from the first tear on, the stipe above the ring lengthens faster than the stipe below it
    // (Hagimoto & Konishi 1959: "mainly at the growth zone ... just beneath the pileus and above
    // the ring"), but the stipe below grows too (Craig et al. 1977), and the cap moves away from
    // the ring: it doesn't ride up with the cap
    const onset = stages.find((s) => fringe(s).length > 0)!;
    const above = (s: Stage) => stipeLength(s) - height(s);
    expect(above(open) / above(onset)).toBeGreaterThan(height(open) / height(onset));
    expect(height(open)).toBeGreaterThan(height(onset));
    for (let i = 1; i < stages.length; i++) {
      expect(above(stages[i]), `t = ${stages[i].t}`).toBeGreaterThanOrEqual(
        above(stages[i - 1]) - 1e-4
      );
    }
    // the ring never sits above the margin of the closed cap
    for (const s of stages.filter((s) => s.t <= 0.4)) {
      const margin = mean(s.capRays.map((r) => r.points.at(-1)!.p[1]));
      const ringAt = mean(ring(s).map((v) => v.points[0].p[1]));
      expect(ringAt, `t = ${s.t}`).toBeLessThan(margin);
    }
  });

  it('gives the veil as a thin closed skin at every stage', () => {
    for (const s of stages) {
      expect(s.skin.faces.length, `t = ${s.t}`).toBeGreaterThan(0);
    }
    for (const t of [0, 0.45, 1]) {
      expect(isClosed(at(t).skin), `t = ${t}`).toBe(true);
    }
    // open, the skin hangs from the ring and the margin and nowhere else: between the ring's
    // edge and the margin there is none of it
    const open = at(1);
    const ys = (from: number, to: number) => {
      let n = 0;
      for (let i = 1; i < open.skin.positions.length; i += 3) {
        if (open.skin.positions[i] > from && open.skin.positions[i] < to) n++;
      }
      return n;
    };
    const ringTop = Math.max(...ring(open).map((v) => v.points[0].p[1]));
    const margin = Math.min(...open.capRays.map((r) => r.points.at(-1)!.p[1]));
    expect(ys(ringTop + 0.003, margin - 0.004)).toBe(0);
    expect(ys(ringTop - 0.01, ringTop + 0.001)).toBeGreaterThan(0);
  });
});
