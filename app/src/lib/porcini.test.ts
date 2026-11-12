// The Porcini grows from a button on a fat stipe into a brown cushion over a sponge of tubes
// that turn from white to yellow-olive, with a white net on the upper part of the stipe. It runs
// the template with the compiled nodes at a coarse resolution.field.
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { BenchmarkRegistry } from '../../benchmark/benchmarkRegistry';
import template from './graph-templates/porcini.json';
import { expandGroups, MemoryRuntimeExecutor } from './runtime/runtime-executor';
import { MemoryRuntimeCache } from './runtime/runtime-executor-cache';

/** A Myzel mesh `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors, ...groups]` */
type Mesh = {
  v: number;
  positions: Float32Array;
  normals: Float32Array;
  colors: Uint32Array;
  groups: Int32Array;
};

function readMesh(data: Int32Array): Mesh {
  const [v, f, flags] = [data[1], data[2], data[3]];
  const start = 4 + 3 * f;
  const colors = flags & 1 ? 7 * v : 6 * v;
  const floats = (from: number, count: number) =>
    new Float32Array(data.buffer, data.byteOffset + from * 4, count);
  return {
    v,
    positions: floats(start, 3 * v),
    normals: floats(start + 3 * v, 3 * v),
    colors: flags & 1
      ? new Uint32Array(data.buffer, data.byteOffset + (start + 6 * v) * 4, v)
      : new Uint32Array(v),
    groups: data.subarray(start + colors, start + colors + v)
  };
}

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
// CI builds the nodes before the unit tests, a local run without `pnpm build:nodes` skips this
const BUILT = existsSync(resolve('static/nodes/max/myzel/pores.wasm'));

type Run = { body: Mesh; netted: Mesh; tubes: Mesh };

const y = (m: Mesh, v: number) => m.positions[v * 3 + 1];
const out = (m: Mesh, v: number) => Math.hypot(m.positions[v * 3], m.positions[v * 3 + 2]);
const vertices = (m: Mesh) => Array.from({ length: m.v }, (_, v) => v);
const channels = (c: number) => [c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff];
const max = (values: number[]) => values.reduce((a, b) => Math.max(a, b), -Infinity);
const min = (values: number[]) => values.reduce((a, b) => Math.min(a, b), Infinity);

describe.skipIf(!BUILT && !ON_CI)('the Porcini', () => {
  const runs = new Map<number, Run>();

  beforeAll(async () => {
    const registry = new BenchmarkRegistry();
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    for (const t of [0, 0.15, 0.5, 1]) {
      const graph = expandGroups(structuredClone(template) as unknown as Graph);
      await registry.load(
        graph.nodes.map((n) => n.type).filter((type) => !type.startsWith('__')) as NodeId[]
      );
      const of = (type: string) => graph.nodes.filter((n) => n.type === type).map((n) => n.id);
      const probes = {
        body: of('max/myzel/hyphae_to_mesh')[0],
        netted: of('max/myzel/cap_pattern')[0],
        tubes: of('max/myzel/set_mesh_color')[0]
      };
      const output = of('max/myzel/output')[0];
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
        'resolution.field': 64
      });
      const debug = runtime.getDebugData();
      runs.set(t, {
        body: readMesh(debug[probes.body].data),
        netted: readMesh(debug[probes.netted].data),
        tubes: readMesh(debug[probes.tubes].data)
      });
    }
  }, 240_000);

  /** The group of the stipe has the lowest vertex, the group of the cap the highest */
  function parts(body: Mesh) {
    const all = vertices(body);
    const lowest = all.reduce((a, b) => (y(body, b) < y(body, a) ? b : a));
    const highest = all.reduce((a, b) => (y(body, b) > y(body, a) ? b : a));
    return { stipe: body.groups[lowest], cap: body.groups[highest] };
  }

  it('is a button on a stipe wider than its cap', () => {
    const { body } = runs.get(0)!;
    const { stipe, cap } = parts(body);
    expect(stipe).not.toBe(cap);
    const widest = (group: number) =>
      max(vertices(body).filter((v) => body.groups[v] === group).map((v) => out(body, v)));
    expect(widest(stipe)).toBeGreaterThan(widest(cap));
    // and an open cushion much wider than its stipe
    const open = runs.get(1)!.body;
    const grown = parts(open);
    const openWidest = (group: number) =>
      max(vertices(open).filter((v) => open.groups[v] === group).map((v) => out(open, v)));
    expect(openWidest(grown.cap)).toBeGreaterThan(1.8 * openWidest(grown.stipe));
  });

  it('hangs the tubes under the cap, deeper and from white to yellow-olive as it grows', () => {
    const extent = (m: Mesh) => {
      const ys = vertices(m).map((v) => y(m, v));
      return max(ys) - min(ys);
    };
    // the closed button has its cap shut around the stipe, the tubes come with the first opening
    expect(runs.get(0)!.tubes.v).toBe(0);
    for (const [t, { body, tubes }] of [...runs].filter(([t]) => t > 0)) {
      expect(tubes.v, `t ${t}`).toBeGreaterThan(1000);
      const { cap } = parts(body);
      const top = max(vertices(body).filter((v) => body.groups[v] === cap).map((v) => y(body, v)));
      const reach = max(
        vertices(body).filter((v) => body.groups[v] === cap).map((v) => out(body, v))
      );
      for (const v of vertices(tubes)) {
        expect(y(tubes, v), `t ${t}`).toBeLessThan(top);
        expect(out(tubes, v), `t ${t}`).toBeLessThan(reach);
      }
    }
    const [young, old] = [runs.get(0.15)!.tubes, runs.get(1)!.tubes];
    expect(extent(old)).toBeGreaterThan(extent(young));
    const [r, g, b] = channels(young.colors[0]);
    expect(Math.min(r, g, b)).toBeGreaterThan(200);
    const [or, og, ob] = channels(old.colors[0]);
    expect(or).toBeGreaterThan(ob + 60);
    expect(og).toBeGreaterThan(ob + 60);
  });

  it('puts a white net on the upper part of the stipe only', () => {
    for (const [t, { body, netted }] of runs) {
      const { stipe } = parts(body);
      const stipeYs = vertices(body).filter((v) => body.groups[v] === stipe).map((v) => y(body, v));
      const [low, high] = [min(stipeYs), max(stipeYs)];
      const changed = vertices(body).filter((v) =>
        [0, 1, 2].some((i) => body.positions[v * 3 + i] !== netted.positions[v * 3 + i])
        || body.colors[v] !== netted.colors[v]
      );
      expect(changed.length, `t ${t}`).toBeGreaterThan(200);
      for (const v of changed) {
        expect(body.groups[v], `t ${t}: vertex ${v}`).toBe(stipe);
        expect((y(body, v) - low) / (high - low), `t ${t}: vertex ${v}`).toBeGreaterThan(0.3);
      }
      const white = changed.filter((v) => min(channels(netted.colors[v])) > 235);
      expect(white.length, `t ${t}`).toBeGreaterThan(50);
    }
  });
});
