// The Fly agaric is a white button whose universal veil tears into warts as the cap grows, the
// red cuticle shows between them. It runs the template with the compiled nodes at a coarse
// resolution.field and compares the body before and after the Cuticle group.
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { BenchmarkRegistry } from '../../benchmark/benchmarkRegistry';
import template from './graph-templates/fly-agaric.json';
import { expandGroups, MemoryRuntimeExecutor } from './runtime/runtime-executor';
import { MemoryRuntimeCache } from './runtime/runtime-executor-cache';

/** A Myzel mesh `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors, ...groups]` */
type Mesh = {
  v: number;
  faces: Int32Array;
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
    faces: data.subarray(4, start),
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
const BUILT = existsSync(resolve('static/nodes/max/myzel/cap_pattern.wasm'));

type Run = { body: Mesh; cuticle: Mesh; bodyWords: Int32Array; cuticleWords: Int32Array };

describe.skipIf(!BUILT && !ON_CI)('the Fly agaric', () => {
  const runs = new Map<string, Run>();

  beforeAll(async () => {
    const registry = new BenchmarkRegistry();
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const run = async (t: number, cap: Record<string, number> = {}) => {
      const source = structuredClone(template) as unknown as Graph;
      const capGroup = source.groups.find((g) => g.name === 'Cap')!;
      Object.assign(source.nodes.find((n) => n.props?.groupId === capGroup.id)!.props!, cap);
      const graph = expandGroups(source);
      await registry.load(
        graph.nodes.map((n) => n.type).filter((type) => !type.startsWith('__')) as NodeId[]
      );
      const of = (type: string) => graph.nodes.filter((n) => n.type === type).map((n) => n.id);
      const output = of('max/myzel/output')[0];
      const probes = {
        body: of('max/myzel/hyphae_to_mesh')[0],
        cuticle: of('max/myzel/cap_pattern')[0]
      };
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
      const [bodyWords, cuticleWords] = [debug[probes.body].data, debug[probes.cuticle].data];
      return {
        body: readMesh(bodyWords),
        cuticle: readMesh(cuticleWords),
        bodyWords,
        cuticleWords
      };
    };
    for (const t of [0, 0.5, 0.75, 1]) runs.set(`${t}`, await run(t));
    runs.set('small', await run(1, { radius_open: 0.045 }));
  }, 240_000);

  const isRed = (c: number) => (c & 0xff) - ((c >> 8) & 0xff) > 60;

  /** The group of the top of the cap: the one the red cuticle is in */
  function capGroup({ cuticle }: Run) {
    for (let v = 0; v < cuticle.v; v++) if (isRed(cuticle.colors[v])) return cuticle.groups[v];
    return -1;
  }

  /** The vertices of the cap that face up, the top the veil covers */
  function top(run: Run) {
    const { cuticle } = run;
    const group = capGroup(run);
    const vertices: number[] = [];
    for (let v = 0; v < cuticle.v; v++) {
      if (cuticle.groups[v] === group && cuticle.normals[v * 3 + 1] > 0.5) vertices.push(v);
    }
    return vertices;
  }

  /** How many warts the top has: islands of vertices the cuticle left white */
  function warts(run: Run) {
    const { cuticle } = run;
    const white = new Set(top(run).filter((v) => !isRed(cuticle.colors[v])));
    const parent = new Map([...white].map((v) => [v, v]));
    const find = (v: number): number => {
      while (parent.get(v) !== v) v = parent.get(v)!;
      return v;
    };
    const { faces } = cuticle;
    for (let i = 0; i < faces.length; i += 3) {
      for (const [a, b] of [[faces[i], faces[i + 1]], [faces[i + 1], faces[i + 2]]]) {
        if (white.has(a) && white.has(b)) parent.set(find(a), find(b));
      }
    }
    const sizes = new Map<number, number>();
    for (const v of white) sizes.set(find(v), (sizes.get(find(v)) ?? 0) + 1);
    // a wart is more than a stray vertex between two cracks
    return [...sizes.values()].filter((size) => size >= 3).length;
  }

  /** The share of the top where the red cuticle shows */
  const red = (run: Run) => {
    const vertices = top(run);
    return vertices.filter((v) => isRed(run.cuticle.colors[v])).length / vertices.length;
  };

  it('is a white egg before the veil tears', () => {
    const { bodyWords, cuticleWords } = runs.get('0')!;
    expect(cuticleWords.length).toBe(bodyWords.length);
    expect(cuticleWords.every((w, i) => w === bodyWords[i])).toBe(true);
  });

  it('keeps the warts and the cuticle on the top of the cap', () => {
    for (const t of ['0.5', '0.75', '1']) {
      const run = runs.get(t)!;
      const { body, cuticle } = run;
      const group = capGroup(run);
      expect(group, `t ${t}`).toBeGreaterThanOrEqual(0);
      const changed: number[] = [];
      for (let v = 0; v < body.v; v++) {
        const moved = [0, 1, 2].some((i) =>
          body.positions[v * 3 + i] !== cuticle.positions[v * 3 + i]
        );
        if (!moved && body.colors[v] === cuticle.colors[v]) continue;
        changed.push(v);
        expect(cuticle.groups[v], `t ${t}: vertex ${v}`).toBe(group);
      }
      expect(changed.length, `t ${t}`).toBeGreaterThan(1000);
      // seen from above the cap is round around the middle of what changed
      const [x, z] = [0, 2].map((i) =>
        changed.reduce((sum, v) => sum + body.positions[v * 3 + i], 0) / changed.length
      );
      const out = (v: number) =>
        Math.hypot(body.positions[v * 3] - x, body.positions[v * 3 + 2] - z);
      const margin = Math.max(...changed.map(out));
      // it faces down only where the margin rolls in under the cap, never over the gills
      for (const v of changed.filter((v) => body.normals[v * 3 + 1] < -0.3)) {
        expect(out(v) / margin, `t ${t}: vertex ${v}`).toBeGreaterThan(0.75);
      }
      expect(cuticle.groups.subarray(body.v).every((g) => g === group), `t ${t}`).toBe(true);
    }
  });

  it('tears the veil open as the cap grows, the warts stay', () => {
    const [half, open, old] = ['0.5', '0.75', '1'].map((t) => red(runs.get(t)!));
    expect(half).toBeLessThan(open);
    expect(open).toBeLessThan(old);
    expect(old).toBeLessThan(0.95);
    expect(warts(runs.get('1')!)).toBeGreaterThan(80);
  });

  it('moves the warts apart as the cap grows', () => {
    // a cap three quarters as wide has about half the area: with warts of a fixed size it would
    // have about half as many
    const ratio = warts(runs.get('small')!) / warts(runs.get('1')!);
    expect(ratio).toBeGreaterThan(0.72);
    expect(ratio).toBeLessThan(1.25);
  });
});
