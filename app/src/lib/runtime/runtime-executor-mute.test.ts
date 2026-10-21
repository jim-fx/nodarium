import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { createMockNodeRegistry } from '../graph-interface/test-utils';
import { MemoryRuntimeExecutor } from './runtime-executor';

type TestNode = Graph['nodes'][0];

function node(
  id: number,
  type: string,
  props?: Record<string, number>,
  muted?: boolean
): TestNode {
  return {
    id,
    type: type as TestNode['type'],
    position: [0, 0],
    ...(props ? { props } : {}),
    ...(muted !== undefined ? { muted } : {})
  };
}

function edge(
  from: number,
  to: number,
  toSocket: string
): [number, number, number, string] {
  return [from, 0, to, toSocket];
}

// emits its `value` prop
const source = {
  id: 'test/node/source',
  inputs: { value: { type: 'integer', value: 0 } },
  outputs: ['integer'],
  execute: ([v]: Int32Array[]) => Int32Array.of(v[0])
} as unknown as NodeDefinition;

// multiplies its first input by 10, the second input is only an offset
const times10 = {
  id: 'test/node/times10',
  inputs: { a: { type: 'integer', value: 0 }, b: { type: 'integer', value: 0 } },
  outputs: ['integer'],
  execute: ([a, b]: Int32Array[]) => Int32Array.of(a[0] * 10 + b[0])
} as unknown as NodeDefinition;

// a + b
const add = {
  id: 'test/node/add',
  inputs: { a: { type: 'integer', value: 0 }, b: { type: 'integer', value: 0 } },
  outputs: ['integer'],
  execute: ([a, b]: Int32Array[]) => Int32Array.of(a[0] + b[0])
} as unknown as NodeDefinition;

const output = {
  id: 'test/node/output',
  inputs: { input: { type: 'integer', value: 0 } },
  outputs: [],
  execute: ([v]: Int32Array[]) => v
} as unknown as NodeDefinition;

function setup() {
  return new MemoryRuntimeExecutor(createMockNodeRegistry([source, times10, add, output]));
}

async function run(graph: Pick<Graph, 'nodes' | 'edges'>) {
  const result = await setup().execute({ id: 0, groups: [], ...graph } as Graph, {});
  return [...(result as Int32Array)];
}

describe('muted nodes in the runtime', () => {
  it('executes a node normally when muted is false or unset', async () => {
    for (const muted of [undefined, false]) {
      const result = await run({
        nodes: [
          node(0, output.id),
          node(1, times10.id, undefined, muted),
          node(2, source.id, { value: 3 })
        ],
        edges: [edge(2, 1, 'a'), edge(1, 0, 'input')]
      });
      expect(result).toEqual([30]);
    }
  });

  it('passes the first input through a muted node: S → [M] → Out', async () => {
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, source.id, { value: 3 })
      ],
      edges: [edge(2, 1, 'a'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([3]);
  });

  it('does not execute the muted node', async () => {
    let calls = 0;
    const counting = { ...times10, execute: () => (calls++, Int32Array.of(0)) };
    const runtime = new MemoryRuntimeExecutor(
      createMockNodeRegistry([source, counting, output])
    );
    await runtime.execute({
      id: 0,
      groups: [],
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, source.id, { value: 3 })
      ],
      edges: [edge(2, 1, 'a'), edge(1, 0, 'input')]
    } as Graph, {});
    expect(calls).toBe(0);
  });

  it('feeds the muted node’s source into every child', async () => {
    // S → [M] → Add.a and Add.b
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, add.id),
        node(2, times10.id, undefined, true),
        node(3, source.id, { value: 4 })
      ],
      edges: [edge(3, 2, 'a'), edge(2, 1, 'a'), edge(2, 1, 'b'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([8]);
  });

  it('keeps the other children of the source intact', async () => {
    // S → [M] → Add.a, S → Add.b
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, add.id),
        node(2, times10.id, undefined, true),
        node(3, source.id, { value: 4 })
      ],
      edges: [edge(3, 2, 'a'), edge(2, 1, 'a'), edge(3, 1, 'b'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([8]);
  });

  it('bypasses a chain of muted nodes', async () => {
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, times10.id, undefined, true),
        node(3, times10.id, undefined, true),
        node(4, source.id, { value: 5 })
      ],
      edges: [edge(4, 3, 'a'), edge(3, 2, 'a'), edge(2, 1, 'a'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([5]);
  });

  it('bypasses a muted node in the middle of a mixed chain', async () => {
    // S → T → [M] → T → Out  => (3*10) → pass → *10 = 300
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id),
        node(2, times10.id, undefined, true),
        node(3, times10.id),
        node(4, source.id, { value: 3 })
      ],
      edges: [edge(4, 3, 'a'), edge(3, 2, 'a'), edge(2, 1, 'a'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([300]);
  });

  it('uses the first input even when it is not the first connected edge', async () => {
    // Edge to `b` is listed before the edge to `a`
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, source.id, { value: 7 }),
        node(3, source.id, { value: 1 })
      ],
      edges: [edge(3, 1, 'b'), edge(2, 1, 'a'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([7]);
  });

  it('falls back to the default when only a non-first input is connected', async () => {
    // only `b` is connected, `a` is empty: nothing to pass through
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, source.id, { value: 7 })
      ],
      edges: [edge(2, 1, 'b'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([0]);
  });

  it('uses the child’s default when the muted node has no input connected', async () => {
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true)
      ],
      edges: [edge(1, 0, 'input')]
    });
    expect(result).toEqual([0]);
  });

  it('only affects the child input the muted node was connected to', async () => {
    // S1 → [M] → Add.a, S2 → Add.b
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, add.id),
        node(2, times10.id, undefined, true),
        node(3, source.id, { value: 4 }),
        node(4, source.id, { value: 100 })
      ],
      edges: [edge(3, 2, 'a'), edge(2, 1, 'a'), edge(4, 1, 'b'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([104]);
  });

  it('ignores muted on the output node', async () => {
    const result = await run({
      nodes: [node(0, output.id, undefined, true), node(1, source.id, { value: 2 })],
      edges: [edge(1, 0, 'input')]
    });
    expect(result).toEqual([2]);
  });

  it('does not run nodes that only fed a muted node’s non-first inputs', async () => {
    // the muted node drops `b`; its source is unreachable and must not break the run
    const result = await run({
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, true),
        node(2, source.id, { value: 7 }),
        node(3, source.id, { value: 1 })
      ],
      edges: [edge(2, 1, 'a'), edge(3, 1, 'b'), edge(1, 0, 'input')]
    });
    expect(result).toEqual([7]);
  });

  it('gives different results for muted and unmuted runs (cache must not mix them)', async () => {
    const runtime = setup();
    const build = (muted: boolean): Graph => ({
      id: 0,
      groups: [],
      nodes: [
        node(0, output.id),
        node(1, times10.id, undefined, muted),
        node(2, source.id, { value: 3 })
      ],
      edges: [edge(2, 1, 'a'), edge(1, 0, 'input')]
    });
    expect([...(await runtime.execute(build(false), {}) as Int32Array)]).toEqual([30]);
    expect([...(await runtime.execute(build(true), {}) as Int32Array)]).toEqual([3]);
    expect([...(await runtime.execute(build(false), {}) as Int32Array)]).toEqual([30]);
  });
});

describe('muted nodes with mismatched types', () => {
  const vec = {
    id: 'test/node/vec',
    inputs: { a: { type: 'vec3', value: 0 } },
    outputs: ['vec3'],
    execute: ([a]: Int32Array[]) => Int32Array.of(a[0] + 1000)
  } as unknown as NodeDefinition;
  const toVec = {
    id: 'test/node/tovec',
    inputs: { a: { type: 'integer', value: 0 } },
    outputs: ['vec3'],
    execute: ([a]: Int32Array[]) => Int32Array.of(a[0])
  } as unknown as NodeDefinition;
  const vecOut = { ...output, inputs: { input: { type: 'vec3', value: 0 } } } as unknown as NodeDefinition;

  it('drops the bypassed value when the child cannot accept the source type', async () => {
    // S(integer) → [toVec muted] → vec(vec3) → Out; the integer must not reach the vec3 input
    const runtime = new MemoryRuntimeExecutor(
      createMockNodeRegistry([source, toVec, vec, vecOut])
    );
    const result = await runtime.execute({
      id: 0,
      groups: [],
      nodes: [
        node(0, output.id),
        node(1, vec.id),
        node(2, toVec.id, undefined, true),
        node(3, source.id, { value: 5 })
      ],
      edges: [edge(3, 2, 'a'), edge(2, 1, 'a'), edge(1, 0, 'input')]
    } as Graph, {});
    expect([...(result as Int32Array)]).toEqual([1000]);
  });
});

describe('muted group instances', () => {
  // group 10: input boundary 11 → times10 (12) → output boundary 13
  const group = {
    id: 10,
    nodes: [
      node(11, '__internal/group/input'),
      node(12, times10.id),
      node(13, '__internal/group/output')
    ],
    edges: [edge(11, 12, 'a'), edge(12, 13, 'out_0')],
    inputs: { in_0: { type: 'integer' } },
    outputs: [{ type: 'integer', label: 'Out' }]
  } as unknown as Graph['groups'][0];

  const build = (muted?: boolean, withSource = true): Graph => ({
    id: 0,
    groups: [group],
    nodes: [
      node(0, output.id),
      node(1, '__internal/group/instance', { groupId: 10 }, muted),
      node(2, source.id, { value: 3 })
    ],
    edges: [...(withSource ? [edge(2, 1, 'in_0')] : []), edge(1, 0, 'input')]
  });

  it('expands an unmuted group', async () => {
    const result = await setup().execute(build(false), {});
    expect([...(result as Int32Array)]).toEqual([30]);
  });

  it('bypasses a muted group instance', async () => {
    const result = await setup().execute(build(true), {});
    expect([...(result as Int32Array)]).toEqual([3]);
  });

  it('uses the default of the child when the muted group has no input connected', async () => {
    const result = await setup().execute(build(true, false), {});
    expect([...(result as Int32Array)]).toEqual([0]);
  });

  it('bypasses a muted group inside another group', async () => {
    // outer group 20: boundary → muted instance of 10 → boundary
    const outer = {
      id: 20,
      nodes: [
        node(21, '__internal/group/input'),
        node(22, '__internal/group/instance', { groupId: 10 }, true),
        node(23, '__internal/group/output')
      ],
      edges: [edge(21, 22, 'in_0'), edge(22, 23, 'out_0')],
      inputs: { in_0: { type: 'integer' } },
      outputs: [{ type: 'integer', label: 'Out' }]
    } as unknown as Graph['groups'][0];
    const graph = build(false);
    graph.groups = [group, outer];
    graph.nodes[1] = node(1, '__internal/group/instance', { groupId: 20 });

    const result = await setup().execute(graph, {});
    expect([...(result as Int32Array)]).toEqual([3]);
  });

  it('bypasses a chain of a muted group and a muted node', async () => {
    const graph = build(true);
    graph.nodes.push(node(5, times10.id, undefined, true));
    graph.edges = [edge(2, 1, 'in_0'), edge(1, 5, 'a'), edge(5, 0, 'input')];

    const result = await setup().execute(graph, {});
    expect([...(result as Int32Array)]).toEqual([3]);
  });
});
