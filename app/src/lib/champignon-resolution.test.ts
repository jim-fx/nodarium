// The groomed Champignon keeps its size when `resolution.field` goes down (#230): the flesh used
// to swell by its cells, the stipe by 40 % at 40. The gills and the veil are meshed by
// `resolution.curve` and don't change at all. The fringe the veil leaves on the margin hangs from
// the flesh at any resolution (#239).
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { BenchmarkRegistry } from '../../benchmark/benchmarkRegistry';
import template from './graph-templates/champignon-groomed.json';
import { expandGroups, MemoryRuntimeExecutor } from './runtime/runtime-executor';
import { MemoryRuntimeCache } from './runtime/runtime-executor-cache';

/** The positions of a Myzel mesh `[14, v, f, flags, ...faces, ...positions, ...]` */
function positions(data: Int32Array) {
  const [v, f] = [data[1], data[2]];
  return new Float32Array(data.buffer, data.byteOffset + (4 + 3 * f) * 4, 3 * v);
}

function faces(data: Int32Array) {
  return data.subarray(4, 4 + 3 * data[2]);
}

type Vec = [number, number, number];
const sub = (a: Vec, b: Vec): Vec => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: Vec, b: Vec) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec, b: Vec): Vec => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0]
];
const along = (
  a: Vec,
  d: Vec,
  s: number
): Vec => [a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s];

/** The closest point to `p` on the triangle `a b c` (Ericson, Real-Time Collision Detection) */
function closest(p: Vec, a: Vec, b: Vec, c: Vec): Vec {
  const [ab, ac, ap] = [sub(b, a), sub(c, a), sub(p, a)];
  const [d1, d2] = [dot(ab, ap), dot(ac, ap)];
  if (d1 <= 0 && d2 <= 0) return a;
  const bp = sub(p, b);
  const [d3, d4] = [dot(ab, bp), dot(ac, bp)];
  if (d3 >= 0 && d4 <= d3) return b;
  const vc = d1 * d4 - d3 * d2;
  if (vc <= 0 && d1 >= 0 && d3 <= 0) return along(a, ab, d1 / (d1 - d3));
  const cp = sub(p, c);
  const [d5, d6] = [dot(ab, cp), dot(ac, cp)];
  if (d6 >= 0 && d5 <= d6) return c;
  const vb = d5 * d2 - d1 * d6;
  if (vb <= 0 && d2 >= 0 && d6 <= 0) return along(a, ac, d2 / (d2 - d6));
  const va = d3 * d6 - d5 * d4;
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) {
    return along(b, sub(c, b), (d4 - d3) / (d4 - d3 + d5 - d6));
  }
  const [v, w] = [vb / (va + vb + vc), vc / (va + vb + vc)];
  return along(along(a, ab, v), ac, w);
}

/** Whether the ray from `p` along `d` goes through the triangle `a b c` (Möller–Trumbore) */
function hits(p: Vec, d: Vec, a: Vec, b: Vec, c: Vec) {
  const [e1, e2] = [sub(b, a), sub(c, a)];
  const h = cross(d, e2);
  const det = dot(e1, h);
  if (Math.abs(det) < 1e-20) return false;
  const s = sub(p, a);
  const u = dot(s, h) / det;
  if (u < 0 || u > 1) return false;
  const q = cross(s, e1);
  const v = dot(d, q) / det;
  return v >= 0 && u + v <= 1 && dot(e2, q) / det > 0;
}

/** How far `p` is from a closed mesh, negative inside: inside where most of three rays cross it an odd number of times */
function signedDistance(triangles: [Vec, Vec, Vec][], p: Vec) {
  let best = Infinity;
  for (const [a, b, c] of triangles) {
    let outside = 0;
    for (let k = 0; k < 3; k++) {
      outside += Math.max(Math.min(a[k], b[k], c[k]) - p[k], 0, p[k] - Math.max(a[k], b[k], c[k]))
        ** 2;
    }
    if (outside >= best * best) continue;
    best = Math.min(best, Math.hypot(...sub(p, closest(p, a, b, c))));
  }
  const rays: Vec[] = [[0.12, 0.99, 0.1], [0.97, -0.2, 0.13], [-0.1, 0.2, 0.97]];
  const odd =
    rays.filter((d) => triangles.filter(([a, b, c]) => hits(p, d, a, b, c)).length % 2).length;
  return odd >= 2 ? -best : best;
}

/**
 * How far the vertices along the attached edge of the fringe (row 0 of every column of its
 * sheets, see `add_sheet`) are from the flesh, in cells of the flesh
 */
function fringeFromFlesh(fringe: Int32Array, flesh: Int32Array, rows: number, resolution: number) {
  const [f, p] = [faces(flesh), positions(flesh)];
  const vertex = (
    data: Float32Array,
    i: number
  ): Vec => [data[3 * i], data[3 * i + 1], data[3 * i + 2]];
  const triangles: [Vec, Vec, Vec][] = [];
  for (let i = 0; i < f.length; i += 3) {
    triangles.push([vertex(p, f[i]), vertex(p, f[i + 1]), vertex(p, f[i + 2])]);
  }
  const [lo, hi] = [[Infinity, Infinity, Infinity], [-Infinity, -Infinity, -Infinity]];
  for (let i = 0; i < p.length; i++) {
    [lo[i % 3], hi[i % 3]] = [Math.min(lo[i % 3], p[i]), Math.max(hi[i % 3], p[i])];
  }
  // the grid of the flesh spans a little more than the flesh, a cell is at least this large
  const cell = Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]) / resolution;
  const q = positions(fringe);
  const attached: number[] = [];
  // every second column is enough, the strands are a fraction of a millimetre apart
  for (let i = 0; i < q.length / 3; i += 2 * rows) {
    attached.push(signedDistance(triangles, vertex(q, i)) / cell);
  }
  return attached;
}

/**
 * The outer radius of the stipe: in a band from 15 to 35 % of the height of the flesh, the mean
 * over the directions around the band's centre of the farthest vertex. The stipe has hollows
 * inside whose faces would count for a mean over all vertices.
 */
function stipeRadius(p: Float32Array) {
  let [lo, hi] = [Infinity, -Infinity];
  for (let i = 1; i < p.length; i += 3) [lo, hi] = [Math.min(lo, p[i]), Math.max(hi, p[i])];
  const band: number[] = [];
  for (let i = 0; i < p.length; i += 3) {
    if (p[i + 1] >= lo + 0.15 * (hi - lo) && p[i + 1] <= lo + 0.35 * (hi - lo)) band.push(i);
  }
  let [x, z] = [0, 0];
  for (const i of band) [x, z] = [x + p[i] / band.length, z + p[i + 2] / band.length];
  const farthest = new Array(32).fill(0);
  for (const i of band) {
    const [dx, dz] = [p[i] - x, p[i + 2] - z];
    const around = Math.floor((Math.atan2(dz, dx) / (2 * Math.PI) + 0.5) * 32) % 32;
    farthest[around] = Math.max(farthest[around], Math.hypot(dx, dz));
  }
  return farthest.reduce((a, b) => a + b, 0) / farthest.length;
}

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
// CI builds the nodes before the unit tests, a local run without `pnpm build:nodes` skips this
const BUILT = existsSync(resolve('static/nodes/max/myzel/hyphae_to_mesh.wasm'));

describe.skipIf(!BUILT && !ON_CI)('the groomed Champignon at a lower resolution.field', () => {
  type Stage = { stipe: number; gills: number; veil: number };
  const stages = new Map<number, Stage>();
  /** How far the attached edge of the fringe is from the flesh, in cells, at t and resolution */
  const fringes = new Map<string, number[]>();

  beforeAll(async () => {
    const registry = new BenchmarkRegistry();
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const base = expandGroups(structuredClone(template) as unknown as Graph);
    await registry.load(
      base.nodes.map((n) => n.type).filter((t) => !t.startsWith('__')) as NodeId[]
    );
    const of = (type: string) => base.nodes.find((n) => n.type === type)!.id;
    const output = of('max/myzel/output');
    // the closed veil of membrane and the ring and fringe of the skirts, joined
    const veil = base.edges.find((e) =>
      e[0] === of('max/myzel/membrane')
      && base.nodes.find((n) => n.id === e[2])!.type === 'max/myzel/join_meshes'
    )![2];
    // the skirt that hangs from the margin
    const fringe = base.edges.find((e) =>
      e[0] === of('max/myzel/cap_margin') && e[3] === 'attachment'
      && base.nodes.find((n) =>
          n.id === e[2]
        )!.type === 'max/myzel/skirt'
    )![2];
    const probes = {
      flesh: of('max/myzel/hyphae_to_mesh'),
      gills: of('max/myzel/lamellae'),
      veil,
      fringe
    };
    const rows = base.settings!['resolution.curve'] as number;
    for (const t of [1, 0.5]) {
      for (const resolution of [128, 40]) {
        const graph = structuredClone(base);
        graph.edges = graph.edges.filter((e) => e[2] !== output);
        let debugId = 1e9;
        for (const id of Object.values(probes)) {
          graph.nodes.push({ id: ++debugId, type: DEBUG_NODE as NodeId, position: [0, 0] });
          graph.edges.push([id, 0, debugId, 'input']);
        }
        await runtime.execute(graph, {
          ...graph.settings,
          'time.t': t,
          seed: 1,
          'resolution.field': resolution
        });
        const debug = runtime.getDebugData();
        fringes.set(
          `${t} ${resolution}`,
          fringeFromFlesh(debug[probes.fringe].data, debug[probes.flesh].data, rows, resolution)
        );
        if (t !== 1) continue;
        stages.set(resolution, {
          stipe: stipeRadius(positions(debug[probes.flesh].data)),
          gills: debug[probes.gills].data[1],
          veil: debug[probes.veil].data[1]
        });
      }
    }
  }, 240_000);

  it('hangs the fringe from the flesh of the margin, within a cell of it or inside', () => {
    for (const [stage, cells] of fringes) {
      expect(cells.length, stage).toBeGreaterThan(20);
      expect(Math.max(...cells), stage).toBeLessThan(1);
    }
  });

  it('keeps the stipe within 10 % of its radius at the template resolution', () => {
    const [fine, coarse] = [stages.get(128)!, stages.get(40)!];
    // 5-10 mm, the groomed stipe is 1-2 cm thick
    expect(fine.stipe).toBeGreaterThan(0.005);
    expect(fine.stipe).toBeLessThan(0.01);
    expect(Math.abs(coarse.stipe / fine.stipe - 1)).toBeLessThan(0.1);
  });

  it('meshes the gills and the veil the same at any resolution.field', () => {
    const [fine, coarse] = [stages.get(128)!, stages.get(40)!];
    expect(fine.gills).toBeGreaterThan(0);
    expect(fine.veil).toBeGreaterThan(0);
    expect(coarse.gills).toBe(fine.gills);
    expect(coarse.veil).toBe(fine.veil);
  });
});
