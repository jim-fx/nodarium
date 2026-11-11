// The groomed Champignon keeps its size when `resolution.field` goes down (#230): the flesh used
// to swell by its cells, the stipe by 40 % at 40. The gills and the veil are meshed by
// `resolution.curve` and don't change at all.
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
    const probes = { flesh: of('max/myzel/hyphae_to_mesh'), gills: of('max/myzel/lamellae'), veil };
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
        'time.t': 1,
        seed: 1,
        'resolution.field': resolution
      });
      const debug = runtime.getDebugData();
      stages.set(resolution, {
        stipe: stipeRadius(positions(debug[probes.flesh].data)),
        gills: debug[probes.gills].data[1],
        veil: debug[probes.veil].data[1]
      });
    }
  }, 120_000);

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
