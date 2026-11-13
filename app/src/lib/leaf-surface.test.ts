// A leaf grown from its veins with the pattern of PLANTARIUM.md (stem, rotate x 90°,
// points_on_path, branch_from_points) is placed like the leaf of the leaf node, so either goes
// into instance. Run with the compiled nodes.
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

type Leaf = { positions: number[][]; normals: number[][]; faces: number };

function readLeaf(data: Int32Array): Leaf {
  // a single geometry is wrapped in one bracket, not in a list
  const [item] = data[2] === 1
    ? [data.subarray(2)]
    : splitNestedArray(data).filter((i) => i[0] === 1);
  const [vertices, faces] = [item[1], item[2]];
  const floats = new Float32Array(item.buffer, item.byteOffset + (4 + faces * 3) * 4, vertices * 6);
  const vec3s = (from: number) =>
    Array.from(
      { length: vertices },
      (_, i) => Array.from(floats.subarray(from + i * 3, from + i * 3 + 3))
    );
  return { positions: vec3s(0), normals: vec3s(vertices * 3), faces };
}

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
const BUILT = existsSync(resolve('static/nodes/max/plantarium/leaf_surface.wasm'));

const OUTPUT = 0;
const STEM = 1;
const FLAT = 2;
const POINTS = 3;
const SIDE_VEINS = 4;
const SURFACE = 5;
const LEAF = 6;
const JOIN = 7;

const graph = {
  id: 1,
  version: 13,
  settings: {},
  groups: [],
  nodes: [
    { id: OUTPUT, type: 'max/plantarium/output', position: [0, 0], props: {} },
    {
      id: STEM,
      type: 'max/plantarium/stem',
      position: [0, 0],
      props: { length: 1, thickness: 0.05, taper: 0.7 }
    },
    {
      id: FLAT,
      type: 'max/plantarium/rotate',
      position: [0, 0],
      props: { axis: 0, angle: Math.PI / 2 }
    },
    {
      id: POINTS,
      type: 'max/plantarium/points_on_path',
      position: [0, 0],
      props: {
        amount: 12,
        lowest_point: 0.08,
        highest_point: 0.9,
        rotation: 180,
        offset_single: 0.3
      }
    },
    {
      id: SIDE_VEINS,
      type: 'max/plantarium/branch_from_points',
      position: [0, 0],
      props: { length: 0.3, thickness: 1, angle: 40, taper: 0.3 }
    },
    { id: SURFACE, type: 'max/plantarium/leaf_surface', position: [0, 0], props: { smooth: 0.1 } },
    { id: LEAF, type: 'max/plantarium/leaf', position: [0, 0], props: {} },
    { id: JOIN, type: 'max/plantarium/join', position: [0, 0], props: {} }
  ],
  edges: [
    [STEM, 0, FLAT, 'plant'],
    [FLAT, 0, POINTS, 'plant'],
    [FLAT, 0, SIDE_VEINS, 'plant'],
    [POINTS, 0, SIDE_VEINS, 'points'],
    [SIDE_VEINS, 0, SURFACE, 'veins'],
    [SURFACE, 0, JOIN, 'a'],
    [LEAF, 0, JOIN, 'b'],
    [JOIN, 0, OUTPUT, 'input']
  ]
} as unknown as Graph;

describe.skipIf(!BUILT && !ON_CI)('a leaf grown from its veins', () => {
  const read: Record<number, Leaf> = {};

  beforeAll(async () => {
    const registry = new FileNodeRegistry(resolve('static/nodes'));
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    const base = expandGroups(graph);
    await registry.load(base.nodes.map((n) => n.type) as NodeId[]);
    const probed = [SURFACE, LEAF];
    probed.forEach((id, i) => {
      base.nodes.push({ id: 1e9 + i, type: DEBUG_NODE as NodeId, position: [0, 0] });
      base.edges.push([id, 0, 1e9 + i, 'input']);
    });
    await runtime.execute(base, { seed: 1 });
    const debug = runtime.getDebugData();
    probed.forEach((id) => (read[id] = readLeaf(debug[id].data)));
  }, 120_000);

  const bounds = (leaf: Leaf) =>
    [0, 1, 2].map((k) => {
      const values = leaf.positions.map((p) => p[k]);
      return [Math.min(...values), Math.max(...values)];
    });

  it('grows along -z from the origin, as long as the leaf of the leaf node', () => {
    const [, , [leafMin, leafMax]] = bounds(read[LEAF]);
    const [, , [min, max]] = bounds(read[SURFACE]);
    // a midrib 1 m long at size 1 is as long as the leaf of the leaf node, the blade reaches a
    // little behind the base and the side veins near the tip a little beyond it
    expect(leafMax).toBeCloseTo(0, 3);
    expect(leafMin).toBeCloseTo(-100, 3);
    expect(Math.abs(max)).toBeLessThan(8);
    expect(min).toBeLessThan(-98);
    expect(min).toBeGreaterThan(-115);
  });

  it('lies around the midrib on x = 0, like the leaf of the leaf node', () => {
    for (const id of [SURFACE, LEAF]) {
      const [[minX, maxX]] = bounds(read[id]);
      expect(Math.abs(minX + maxX), `leaf ${id}`).toBeLessThan(5);
      expect(maxX, `leaf ${id}`).toBeGreaterThan(20);
    }
  });

  it('faces up, like the leaf of the leaf node', () => {
    for (const id of [SURFACE, LEAF]) {
      const { normals } = read[id];
      const up = normals.reduce((sum, n) => sum + n[1], 0) / normals.length;
      expect(up, `leaf ${id}`).toBeGreaterThan(0.9);
    }
  });
});
