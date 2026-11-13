// The stipe of the groomed mushrooms: strands fill a guide, twist turns them around it,
// random_break ends some of them short of the tip and clump bundles them by guide. Run with the
// compiled nodes.
import {
  expandGroups,
  FileNodeRegistry,
  MemoryRuntimeCache,
  MemoryRuntimeExecutor
} from '@nodarium/runtime/node';
import { DEBUG_NODE, type Graph, type NodeId } from '@nodarium/types';
import { splitNestedArray } from '@nodarium/utils';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';

type Path = {
  id: number;
  parent: number;
  creator: number;
  index: number;
  points: { p: [number, number, number]; r: number }[];
};

function readPaths(data: Int32Array): Path[] {
  return splitNestedArray(data)
    .filter((item) => item[0] === 0 && item.length > 7)
    .map((item) => {
      const floats = new Float32Array(item.buffer, item.byteOffset + 7 * 4, item.length - 7);
      const points: Path['points'] = [];
      for (let i = 0; i + 3 < floats.length; i += 4) {
        points.push({ p: [floats[i], floats[i + 1], floats[i + 2]], r: Math.abs(floats[i + 3]) });
      }
      return { id: item[2], parent: item[3], creator: item[5], index: item[6], points };
    });
}

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
const BUILT = existsSync(resolve('static/nodes/max/plantarium/twist.wasm'));

const STEM = 1;
const STRANDS = 2;
const TWIST = 3;
const BREAK = 4;
const CLUMP = 5;
const OUTPUT = 0;

const graph = (turns: number): Graph =>
  ({
    id: 1,
    version: 10,
    settings: {},
    groups: [],
    nodes: [
      { id: OUTPUT, type: 'max/myzel/output', position: [0, 0], props: {} },
      {
        id: STEM,
        type: 'max/plantarium/stem',
        position: [0, 0],
        props: { length: 0.05, thickness: 0.007, taper: 0.25 }
      },
      {
        id: STRANDS,
        type: 'max/myzel/strands',
        position: [0, 0],
        props: { amount: 240, thickness: 0.0005, wobble: 0.25 }
      },
      { id: TWIST, type: 'max/plantarium/twist', position: [0, 0], props: { turns } },
      {
        id: BREAK,
        type: 'max/plantarium/random_break',
        position: [0, 0],
        props: { chance: 0.5, lowest_break: 0.85, highest_break: 1 }
      },
      {
        id: CLUMP,
        type: 'max/plantarium/clump',
        position: [0, 0],
        props: { strength: 0.5, clumps: 12 }
      }
    ],
    edges: [
      [STEM, 0, STRANDS, 'plant'],
      [STRANDS, 0, TWIST, 'plant'],
      [TWIST, 0, BREAK, 'plant'],
      [BREAK, 0, CLUMP, 'plant']
    ]
  }) as unknown as Graph;

describe.skipIf(!BUILT && !ON_CI)('the strands of a stipe', () => {
  const read: Record<number, Path[]> = {};

  beforeAll(async () => {
    const registry = new FileNodeRegistry(resolve('static/nodes'));
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const base = expandGroups(graph(0.5));
    await registry.load(base.nodes.map((n) => n.type) as NodeId[]);
    const probed = [STEM, STRANDS, TWIST, BREAK, CLUMP];
    probed.forEach((id, i) => {
      base.nodes.push({ id: 1e9 + i, type: DEBUG_NODE as NodeId, position: [0, 0] });
      base.edges.push([id, 0, 1e9 + i, 'input']);
    });
    await runtime.execute(base, { seed: 1 });
    const debug = runtime.getDebugData();
    probed.forEach((id) => (read[id] = readPaths(debug[id].data)));
  }, 120_000);

  const length = (a: number[], b: number[]) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

  it('keep their ids, parent and place in the list through twist, random_break and clump', () => {
    const grown = read[STRANDS];
    expect(grown).toHaveLength(240);
    for (const id of [TWIST, BREAK, CLUMP]) {
      expect(read[id].map((p) => [p.id, p.parent, p.creator, p.index])).toEqual(
        grown.map((p) => [p.id, p.parent, p.creator, p.index])
      );
    }
    expect(new Set(grown.map((p) => p.parent)).size).toBe(1);
  });

  it('are twisted inside the tube of their guide', () => {
    const [guide] = read[STEM];
    const twisted = read[TWIST];
    expect(twisted[0].points).toHaveLength(guide.points.length);
    let moved = 0;
    for (const [k, strand] of twisted.entries()) {
      strand.points.forEach((q, i) => {
        const g = guide.points[i];
        // a twist keeps a strand inside the tube, up to 1% of the tube for the line of the bundle
        expect(length(q.p, g.p) + q.r, `strand ${k} at point ${i}`).toBeLessThanOrEqual(
          g.r * 1.01 + 1e-6
        );
        moved = Math.max(moved, length(q.p, read[STRANDS][k].points[i].p));
      });
    }
    // half a turn puts the tips on the other side of the guide
    expect(moved).toBeGreaterThan(0.003);
    // the base of every strand stays
    twisted.forEach((s, k) => {
      expect(length(s.points[0].p, read[STRANDS][k].points[0].p)).toBeLessThan(1e-6);
    });
  });

  it('end short of the tip when they are broken, and only then', () => {
    const tip = (path: Path) => path.points.at(-1)!.p[1];
    const top = Math.max(...read[TWIST].map(tip));
    const shortened = read[BREAK].filter((p, k) =>
      length(p.points.at(-1)!.p, read[TWIST][k].points.at(-1)!.p) > 1e-4
    );
    // half of them break, between 85 % and 100 % of their length
    expect(shortened.length).toBeGreaterThan(80);
    expect(shortened.length).toBeLessThan(160);
    for (const path of read[BREAK]) {
      expect(tip(path)).toBeGreaterThan(0.84 * top);
    }
  });
});
