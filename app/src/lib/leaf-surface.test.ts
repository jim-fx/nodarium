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

describe.skipIf(!BUILT && !ON_CI)('leaf variants from one vein graph', () => {
  const RANDOM = 8;
  const PLANT = 9;
  const INSTANCE = 10;
  let leaves: Int32Array[] = [];
  let placed: Int32Array[] = [];

  beforeAll(async () => {
    const variants = structuredClone(graph);
    const node = (id: number) => variants.nodes.find((n) => n.id === id)!;
    node(STEM).props = { ...node(STEM).props, amount: 3 };
    node(SURFACE).props = { ...node(SURFACE).props, variants: 3 };
    variants.nodes.push(
      {
        id: RANDOM,
        type: 'max/plantarium/random',
        position: [0, 0],
        props: { min: 0.6, max: 1.4 }
      },
      {
        id: PLANT,
        type: 'max/plantarium/stem',
        position: [0, 0],
        props: { amount: 1, length: 2, thickness: 0.05 }
      },
      {
        id: INSTANCE,
        type: 'max/plantarium/instance',
        position: [0, 0],
        props: { amount: 60, lowest_instance: 0, highest_instance: 1 }
      } as Graph['nodes'][number]
    );
    variants.edges = variants.edges.filter((e) => e[2] !== JOIN && e[0] !== JOIN);
    variants.nodes = variants.nodes.filter((n) => n.id !== JOIN && n.id !== LEAF);
    variants.edges.push(
      [RANDOM, 0, STEM, 'length'],
      [PLANT, 0, INSTANCE, 'plant'],
      [SURFACE, 0, INSTANCE, 'geometry'],
      [INSTANCE, 0, OUTPUT, 'input']
    );

    const registry = new FileNodeRegistry(resolve('static/nodes'));
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    await registry.load(variants.nodes.map((n) => n.type) as NodeId[]);
    variants.nodes.push({ id: 1e9, type: DEBUG_NODE as NodeId, position: [0, 0] });
    variants.edges.push([SURFACE, 0, 1e9, 'input']);
    const result = await runtime.execute(variants, { seed: 1 });
    leaves = splitNestedArray(runtime.getDebugData()[SURFACE].data);
    placed = splitNestedArray(result).filter((i) => i[0] === 2);
  }, 120_000);

  it('makes a leaf of every midrib, each its own', () => {
    expect(leaves).toHaveLength(3);
    expect(leaves.every((l) => l[0] === 1)).toBe(true);
    const lengths = leaves.map((l) => readLeaf(Int32Array.from([0, l.length + 1, ...l, 1, 1])))
      .map((leaf) => Math.min(...leaf.positions.map((p) => p[2])));
    expect(new Set(lengths.map((l) => l.toFixed(1))).size).toBe(3);
  });

  it('lets instance pick among them', () => {
    expect(placed).toHaveLength(3);
    expect(placed.reduce((sum, i) => sum + i[3], 0)).toBe(60);
  });
});

describe.skipIf(!BUILT && !ON_CI)('deform on the leaves of leaf_surface and leaf', () => {
  const TWIST = 8;
  const CURL = 9;
  let leaves: Int32Array[] = [];
  let twisted: Int32Array[] = [];
  let flat: Leaf;
  let curled: Leaf;

  const wrap = (item: Int32Array) => readLeaf(Int32Array.from([0, item.length + 1, ...item, 1, 1]));

  beforeAll(async () => {
    const deformed = structuredClone(graph);
    const node = (id: number) => deformed.nodes.find((n) => n.id === id)!;
    node(STEM).props = { ...node(STEM).props, amount: 3 };
    node(SURFACE).props = { ...node(SURFACE).props, variants: 3 };
    node(LEAF).props = { x_resolution: 8 };
    deformed.nodes.push(
      {
        id: TWIST,
        type: 'max/plantarium/deform',
        position: [0, 0],
        props: { mode: 2, amount: 0.25 }
      },
      {
        id: CURL,
        type: 'max/plantarium/deform',
        position: [0, 0],
        props: { mode: 1, amount: -0.25 }
      }
    );
    deformed.edges = deformed.edges.filter((e) => e[2] !== JOIN);
    deformed.edges.push(
      [SURFACE, 0, TWIST, 'geometry'],
      [LEAF, 0, CURL, 'geometry'],
      [TWIST, 0, JOIN, 'a'],
      [CURL, 0, JOIN, 'b']
    );

    const registry = new FileNodeRegistry(resolve('static/nodes'));
    const runtime = new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
    await registry.load(deformed.nodes.map((n) => n.type) as NodeId[]);
    const probed = [SURFACE, TWIST, LEAF, CURL];
    probed.forEach((id, i) => {
      deformed.nodes.push({ id: 1e9 + i, type: DEBUG_NODE as NodeId, position: [0, 0] });
      deformed.edges.push([id, 0, 1e9 + i, 'input']);
    });
    await runtime.execute(deformed, { seed: 1 });
    const debug = runtime.getDebugData();
    leaves = splitNestedArray(debug[SURFACE].data);
    twisted = splitNestedArray(debug[TWIST].data);
    flat = readLeaf(debug[LEAF].data);
    curled = readLeaf(debug[CURL].data);
  }, 120_000);

  it('twists every leaf of the list and keeps its vertices', () => {
    expect(twisted).toHaveLength(3);
    twisted.forEach((leaf, i) => {
      expect([leaf[1], leaf[2]]).toEqual([leaves[i][1], leaves[i][2]]);
      // a quarter turn stands the tip on its edge
      const { positions, normals } = wrap(leaf);
      const length = -Math.min(...positions.map((p) => p[2]));
      const tip = positions.filter((p) => p[2] < -0.85 * length);
      const reach = (k: number) => Math.max(...tip.map((p) => Math.abs(p[k])));
      expect(reach(1)).toBeGreaterThan(2 * reach(0));
      expect(normals.every((n) => Math.abs(Math.hypot(...n) - 1) < 1e-3)).toBe(true);
    });
  });

  it('rolls the margins of the leaf node under', () => {
    expect(curled.positions).toHaveLength(flat.positions.length);
    const width = (leaf: Leaf) => Math.max(...leaf.positions.map((p) => Math.abs(p[0])));
    expect(width(curled)).toBeLessThan(0.9 * width(flat));
    expect(Math.min(...curled.positions.map((p) => p[1]))).toBeLessThan(
      Math.min(...flat.positions.map((p) => p[1])) - 5
    );
  });
});
