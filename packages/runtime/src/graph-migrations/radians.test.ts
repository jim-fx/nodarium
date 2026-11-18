import type { Graph, SerializedNode } from '@nodarium/types';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { parseProjectFile } from '../project-file';
import { FileNodeRegistry } from '../registry/file-node-registry';
import { MemoryRuntimeExecutor } from '../runtime-executor';
import before from './fixtures/radians-before.json';
import v13 from './fixtures/v13.json';
import { type GeometrySummary, geometrySummary } from './geometry-summary';
import { migrateGraph } from './index';
import { radians } from './radians';

const nodesDir = fileURLToPath(new URL('../../../../app/static/nodes', import.meta.url));
const templatesDir = new URL('../../templates/', import.meta.url);
const registry = new FileNodeRegistry(nodesDir);

async function build(graph: Graph) {
  const runtime = new MemoryRuntimeExecutor(registry);
  try {
    return geometrySummary(await runtime.execute(graph, { ...graph.settings, seed: 1 }));
  } finally {
    runtime.releaseInstances();
  }
}

/** Whether two builds made the same plant, up to the rounding of the converted angles */
function expectSame(after: GeometrySummary, old: GeometrySummary, name: string) {
  expect(after.triangles, name).toBe(old.triangles);
  const extent = Math.max(...old.max.map((v, i) => v - old.min[i]), 1e-9);
  for (const key of ['min', 'max', 'center', 'spread'] as const) {
    for (let d = 0; d < 3; d++) {
      expect(Math.abs(after[key][d] - old[key][d]) / extent, `${name} ${key}[${d}]`)
        .toBeLessThan(1e-4);
    }
  }
}

function node(id: number, type: string, props: SerializedNode['props'] = {}): SerializedNode {
  return { id, type: `max/plantarium/${type}` as SerializedNode['type'], props };
}

function graph(nodes: SerializedNode[], edges: Graph['edges']): Graph {
  return { id: 1, version: 13, nodes, edges, groups: [] };
}

const recordedTemplates = Object.entries(before as Record<string, GeometrySummary>)
  .filter(([name]) => name.startsWith('templates/'))
  .map(([name, old]) => [name.slice('templates/'.length), old] as const);

describe('every angle in radians (graph version 14)', () => {
  // one test per template: all of them in one test take longer than a test may on CI
  it.each(recordedTemplates)('builds %s as it was built with degrees', async (name, old) => {
    const file = new URL(`${name}.json`, templatesDir);
    const template = parseProjectFile(JSON.parse(readFileSync(file, 'utf8'))).graph;
    expectSame(await build(template), old, name);
  });

  it('builds a graph of version 13 as it was built with degrees', async () => {
    const migrated = parseProjectFile(structuredClone(v13)).graph;
    expect(migrated.version).toBeGreaterThanOrEqual(14);
    expectSame(
      await build(migrated),
      (before as Record<string, GeometrySummary>)['fixtures/v13'],
      'v13'
    );
  });

  it('converts values set on a node and leaves the others', () => {
    const old = graph([
      node(1, 'branch', { angle: 90, rotation: 180, length: 2 }),
      node(2, 'deform', { amount: 0.25 }),
      node(3, 'instance', { rotation: 2.4 })
    ], []);
    const [branch, deform, instance] = radians(old).nodes;
    expect(branch.props).toEqual({ angle: 1.5707963, rotation: 3.1415927, length: 2 });
    expect(deform.props!.amount).toBeCloseTo(Math.PI / 2, 6);
    expect(instance.props).toEqual({ rotation: 2.4 });
  });

  it('scales a random, a curve or a float that only feeds angles', () => {
    const old = graph([
      node(1, 'random', { min: 10, max: 30 }),
      node(2, 'curve', {}),
      node(3, 'float', { value: 45 }),
      node(4, 'branch'),
      node(5, 'instance'),
      node(6, 'trunk_kink')
    ], [[1, 0, 4, 'angle'], [1, 0, 5, 'angle'], [2, 0, 4, 'rotation'], [3, 0, 6, 'angle']]);
    const migrated = radians(old);
    const props = (id: number) => migrated.nodes.find((n) => n.id === id)!.props;
    expect(props(1)).toEqual({ min: 0.1745329, max: 0.5235988 });
    // the defaults of the curve, 0 and 1, are scaled too
    expect(props(2)).toEqual({ min: 0, max: 0.0174533 });
    expect(props(3)).toEqual({ value: 0.7853982 });
    expect(migrated.nodes).toHaveLength(6);
    expect(migrated.edges).toEqual(old.edges);
  });

  it('puts a math node between a source that feeds more than angles and the angles', () => {
    const old = graph([
      node(1, 'random', { min: 10, max: 30 }),
      node(2, 'branch'),
      node(3, 'vec3'),
      node(4, 'deform'),
      node(5, 'curve', {}),
      node(6, 'random', { min: 0, max: 1 })
    ], [
      [1, 0, 2, 'angle'],
      [1, 0, 3, 'z'],
      [5, 0, 2, 'rotation'],
      [5, 0, 4, 'amount'],
      [6, 0, 5, 'min']
    ]);
    const migrated = radians(old);
    // the random keeps its range for the vec3
    expect(migrated.nodes.find((n) => n.id === 1)!.props).toEqual({ min: 10, max: 30 });
    const maths = migrated.nodes.filter((n) => n.type === 'max/plantarium/math');
    expect(maths.map((m) => m.props)).toEqual([
      { op_type: 2, b: Math.PI / 180 },
      { op_type: 2, b: Math.PI / 180 },
      { op_type: 2, b: 2 * Math.PI }
    ]);
    const [a, b, c] = maths.map((m) => m.id);
    expect(migrated.edges).toEqual(expect.arrayContaining([
      [1, 0, a, 'a'],
      [a, 0, 2, 'angle'],
      [1, 0, 3, 'z'],
      [5, 0, b, 'a'],
      [b, 0, 2, 'rotation'],
      [5, 0, c, 'a'],
      [c, 0, 4, 'amount'],
      [6, 0, 5, 'min']
    ]));
    expect(migrated.edges).toHaveLength(8);
  });

  it('converts the angles inside groups, also those a group input feeds', () => {
    const old = graph([], []);
    old.groups = [{
      id: 10,
      name: 'Leaf',
      inputs: { tilt: { type: 'float', value: 30 } },
      outputs: {},
      nodes: [
        { id: 11, type: '__internal/group/input', props: {} } as SerializedNode,
        node(12, 'branch_from_points', { angle: 30 }),
        node(13, 'points_on_path', { rotation: 180 })
      ],
      edges: [[11, 0, 12, 'angle']]
    }] as unknown as Graph['groups'];
    const group = migrateGraph(old).groups![0];
    expect(group.nodes.find((n) => n.id === 13)!.props).toEqual({ rotation: 3.1415927 });
    const math = group.nodes.find((n) => n.type === 'max/plantarium/math')!;
    expect(math.id).toBeGreaterThan(13);
    expect(group.edges).toEqual([[math.id, 0, 12, 'angle'], [11, 0, math.id, 'a']]);
  });
});
