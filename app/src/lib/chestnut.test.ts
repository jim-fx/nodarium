// The Chestnut mushroom is the groomed Champignon with a brown cap and a Cuticle group: fine
// scales that open towards the margin as the cap grows, and cracks on the old cap. It runs the
// template with the compiled nodes at a coarse resolution.field and compares the body before and
// after the Cuticle group.
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { BenchmarkRegistry } from '../../benchmark/benchmarkRegistry';
import template from './graph-templates/chestnut.json';
import { expandGroups, MemoryRuntimeExecutor } from './runtime/runtime-executor';
import { MemoryRuntimeCache } from './runtime/runtime-executor-cache';

/** A Myzel mesh `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors, ...groups]` */
type Mesh = { v: number; positions: Float32Array; colors: Uint32Array; groups: Int32Array };

function readMesh(data: Int32Array): Mesh {
  const [v, f, flags] = [data[1], data[2], data[3]];
  const start = 4 + 3 * f;
  const colors = flags & 1 ? 7 * v : 6 * v;
  return {
    v,
    positions: new Float32Array(data.buffer, data.byteOffset + start * 4, 3 * v),
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

describe.skipIf(!BUILT && !ON_CI)('the Chestnut mushroom', () => {
  const runs = new Map<string, Run>();
  const cuticleGroup = (template as unknown as Graph).groups.find((g) => g.name === 'Cuticle')!;
  const flesh = cuticleGroup.inputs!.color.value as number[];

  beforeAll(async () => {
    const registry = new BenchmarkRegistry();
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const run = async (t: number, cuticle: Record<string, number> = {}) => {
      const source = structuredClone(template) as unknown as Graph;
      const instance = source.nodes.find((n) =>
        n.props?.groupId === source.groups.find((g) => g.name === 'Cuticle')!.id
      )!;
      Object.assign(instance.props!, cuticle);
      const graph = expandGroups(source);
      await registry.load(
        graph.nodes.map((n) => n.type).filter((type) => !type.startsWith('__')) as NodeId[]
      );
      const of = (type: string) => graph.nodes.filter((n) => n.type === type).map((n) => n.id);
      const output = of('max/myzel/output')[0];
      // the last cap_pattern is the one that goes on to join_meshes
      const patterns = of('max/myzel/cap_pattern');
      const last = patterns.find((id) =>
        !graph.edges.some((e) => e[0] === id && patterns.includes(e[2]))
      )!;
      const probes = { body: of('max/myzel/hyphae_to_mesh')[0], cuticle: last };
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
        'resolution.field': 48
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
    runs.set('young', await run(0.5));
    runs.set('open', await run(0.75));
    runs.set('old', await run(1));
    runs.set('smooth', await run(1, { scales: 0, cracks: 0 }));
  }, 180_000);

  /** How far the cap group has gone from the brown of the cap to the pale flesh, 0 to 1 */
  const paleness = (run: Run) => {
    const { body, cuticle } = run;
    const group = capGroup(run);
    const channels = (c: number) => [c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff];
    let [sum, all] = [0, 0];
    for (let v = 0; v < body.v; v++) {
      if (cuticle.groups[v] !== group) continue;
      const [before, after] = [channels(body.colors[v]), channels(cuticle.colors[v])];
      const span = [0, 1, 2].reduce((a, i) => a + flesh[i] - before[i], 0);
      sum += [0, 1, 2].reduce((a, i) => a + after[i] - before[i], 0) / span;
      all++;
    }
    return sum / all;
  };

  /** The group whose vertices the cuticle changed */
  function capGroup({ body, cuticle }: Run) {
    for (let v = 0; v < body.v; v++) {
      if (body.colors[v] !== cuticle.colors[v]) return body.groups[v];
    }
    return -1;
  }

  it('changes only the top of the cap, one group of the body', () => {
    for (const [name, run] of runs) {
      if (name === 'smooth') continue;
      const { body, cuticle } = run;
      const group = capGroup(run);
      expect(group, name).toBeGreaterThanOrEqual(0);
      let changed = 0;
      for (let v = 0; v < body.v; v++) {
        const moved = [0, 1, 2].some((i) =>
          body.positions[v * 3 + i] !== cuticle.positions[v * 3 + i]
        );
        if (!moved && body.colors[v] === cuticle.colors[v]) continue;
        changed++;
        expect(cuticle.groups[v], `${name}: vertex ${v}`).toBe(group);
      }
      expect(changed, name).toBeGreaterThan(100);
      // the vertices added to the top are in its group
      expect(cuticle.groups.subarray(body.v).every((g) => g === group), name).toBe(true);
    }
  });

  it('opens the cuticle as the cap grows old', () => {
    const [young, open, old] = ['young', 'open', 'old'].map((k) => paleness(runs.get(k)!));
    expect(young).toBeLessThan(open);
    expect(open).toBeLessThan(old);
    // the old cap is cracked all over but still mostly brown
    expect(old).toBeGreaterThan(0.08);
    expect(old).toBeLessThan(0.5);
  });

  it('leaves the body as it is without scales and cracks', () => {
    const { bodyWords, cuticleWords } = runs.get('smooth')!;
    expect(cuticleWords.length).toBe(bodyWords.length);
    expect(cuticleWords.every((w, i) => w === bodyWords[i])).toBe(true);
  });
});
