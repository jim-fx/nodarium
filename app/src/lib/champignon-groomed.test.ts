// The groomed Champignon template matures with time: every stage from the button (t = 0) to the
// open cap (t = 1) is checked in steps of 0.05 with the compiled nodes, as the research in #175
// asks. It runs the guides only (stipe, cap rays) and the meshes of the gills and the veil, not hyphae_to_mesh.
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
  skin: Mesh;
  gills: Mesh;
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
      capRays: [cap, 0]
    } as const;
    // the closed veil of membrane and the ring and fringe of the skirts, joined
    const veil = base.edges.find((e) =>
      e[0] === of('max/myzel/membrane')
      && base.nodes.find((n) => n.id === e[2])!.type === 'max/myzel/join_meshes'
    )![2];
    const meshes = {
      skin: [veil, 0],
      gills: [of('max/myzel/lamellae'), 0]
    } as const;
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
      const shapes = await run(t, meshes);
      const stipe = read('stipe')[0];
      const [top, below] = [stipe.points.at(-1)!.p, stipe.points.at(-2)!.p];
      const direction = sub(top, below);
      const capRays = read('capRays');
      // the stipe runs along with the rays, they grow on it
      const rays = read('rays').filter((r) => r.parent === stipe.id);
      stages.push({
        t,
        stipe,
        rays,
        // the rays the cap node grows run from the axis, fill_gaps adds more further out
        capRays: rays.filter((r) => capRays.some((c) => c.id === r.id)),
        skin: readMesh(shapes[meshes.skin[0]].data),
        gills: readMesh(shapes[meshes.gills[0]].data),
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

  it('keeps the gills under the cap at every stage', () => {
    for (const s of stages) {
      const reach = Math.max(...s.rays.flatMap((r) => r.points.map((q) => plane(s, q.p)[0])));
      const top = Math.max(...s.rays.flatMap((r) => r.points.map((q) => plane(s, q.p)[1])));
      expect(s.gills.faces.length, `t = ${s.t}`).toBeGreaterThan(0);
      let [widest, highest] = [0, -Infinity];
      for (let i = 0; i < s.gills.positions.length; i += 3) {
        const [r, h] = plane(s, Array.from(s.gills.positions.subarray(i, i + 3)) as Vec);
        widest = Math.max(widest, r);
        highest = Math.max(highest, h);
      }
      expect(widest, `t = ${s.t}`).toBeLessThan(reach);
      expect(highest, `t = ${s.t}`).toBeLessThan(top);
    }
  });

  it('closes the veil between the margin and the stipe on the button, a ring in the upper third when open', () => {
    const radii = (s: Stage) => {
      const r: number[] = [];
      for (let i = 0; i < s.skin.positions.length; i += 3) {
        r.push(plane(s, Array.from(s.skin.positions.subarray(i, i + 3)) as Vec)[0]);
      }
      return r;
    };
    // closed: from the stipe out to the margin
    const button = at(0);
    const margin = mean(button.capRays.map((r) => plane(button, r.points.at(-1)!.p)[0]));
    expect(Math.min(...radii(button))).toBeLessThan(button.stipe.points.at(-1)!.r);
    expect(Math.max(...radii(button))).toBeGreaterThan(0.95 * margin);
    // open: the top of the ring, where the skin meets the stipe, is in the upper third of it
    const open = at(1);
    const stipeRadius = Math.max(...open.stipe.points.map((q) => q.r));
    let ringTop = -Infinity;
    for (let i = 0; i < open.skin.positions.length; i += 3) {
      const [r, h] = plane(open, Array.from(open.skin.positions.subarray(i, i + 3)) as Vec);
      if (r < stipeRadius) ringTop = Math.max(ringTop, h);
    }
    const fromBase = stipeLength(open) + ringTop;
    expect(fromBase / stipeLength(open)).toBeGreaterThan(2 / 3);
    expect(fromBase / stipeLength(open)).toBeLessThan(0.9);
  });

  it('gives the veil as a thin closed skin at every stage', () => {
    for (const s of stages) {
      expect(s.skin.faces.length, `t = ${s.t}`).toBeGreaterThan(0);
    }
    for (const t of [0, 0.45, 1]) {
      expect(isClosed(at(t).skin), `t = ${t}`).toBe(true);
    }
    // open, the skin hangs from the ring and the margin and nowhere else: there is a gap between
    // the lowest vertex of the part on the margin and the highest of the ring
    const heights: number[] = [];
    for (let i = 1; i < at(1).skin.positions.length; i += 3) heights.push(at(1).skin.positions[i]);
    heights.sort((a, b) => a - b);
    const gap = Math.max(...heights.slice(1).map((h, i) => h - heights[i]));
    expect(gap).toBeGreaterThan(0.004);
  });
});
