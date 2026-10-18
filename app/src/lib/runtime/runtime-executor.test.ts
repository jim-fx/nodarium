import type { Graph, NodeDefinition, NodeValue } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { createMockNodeRegistry } from '../graph-interface/test-utils';
import { expandGroups, MemoryRuntimeExecutor } from './runtime-executor';

// Helpers to build minimal serialized nodes/edges
function node(id: number, type: string, props?: Record<string, number>) {
  return {
    id,
    type: type as Graph['nodes'][0]['type'],
    position: [0, 0] as [number, number],
    ...(props ? { props } : {})
  };
}

function edge(
  from: number,
  fromSocket: number,
  to: number,
  toSocket: string
): [number, number, number, string] {
  return [from, fromSocket, to, toSocket];
}

// Expanded nodes get hashed IDs, so tests find them by their wiring
function sourceOf(graph: Graph, id: number, socket?: string) {
  return graph.edges.find(e => e[2] === id && (socket === undefined || e[3] === socket))![0];
}

function targetOf(graph: Graph, id: number) {
  return graph.edges.find(e => e[0] === id)![2];
}

describe('expandGroups', () => {
  it('returns graph unchanged when there are no groups', () => {
    const graph: Graph = {
      id: 1,
      nodes: [node(0, 'test/node/output'), node(1, 'test/node/input')],
      edges: [edge(0, 0, 1, 'value')],
      groups: []
    };

    const result = expandGroups(graph);

    expect(result.nodes.length).toBe(2);
    expect(result.edges.length).toBe(1);
    expect(result).toBe(graph); // same reference — no copy needed
  });

  it('expands a simple group: A → [B] → C rewires to A → B → C', () => {
    // IDs: A=1, B=2, C=3, groupNode=4, group.id=5, inputBoundary=6, outputBoundary=7
    const groupId = 5;
    const groupNodeId = 4;

    const graph: Graph = {
      id: 1,
      nodes: [
        node(1, 'test/node/output'),
        node(groupNodeId, '__internal/group/instance', { groupId }),
        node(3, 'test/node/input')
      ],
      edges: [
        edge(1, 0, groupNodeId, 'input_0'), // A → group
        edge(groupNodeId, 0, 3, 'value') // group → C
      ],
      groups: [{
        id: groupId,
        nodes: [
          node(6, '__internal/group/input'),
          node(2, 'test/node/output'),
          node(7, '__internal/group/output')
        ],
        edges: [
          edge(6, 0, 2, 'input'), // inputBoundary → B
          edge(2, 0, 7, 'Out') // B → outputBoundary
        ],
        inputs: { input_0: { type: 'float' } },
        outputs: [{ type: 'float', label: 'Output 0' }]
      }]
    };

    const result = expandGroups(graph);
    const remappedB = targetOf(result, 1);

    const ids = result.nodes.map(n => n.id);
    expect(ids).not.toContain(groupNodeId);
    expect(ids).not.toContain(2);
    expect(ids).toContain(remappedB);
    expect(ids).toContain(1); // A
    expect(ids).toContain(3); // C
    expect(result.nodes.length).toBe(3); // A, B(remapped), C

    expect(result.edges).toContainEqual(edge(1, 0, remappedB, 'input')); // A → B
    expect(result.edges).toContainEqual(edge(remappedB, 0, 3, 'value')); // B → C
    expect(result.edges.length).toBe(2);
  });

  it('expands a group with two internal nodes (B→D) and preserves the internal edge', () => {
    // A → [B → D] → C
    const groupId = 10;
    const groupNodeId = 5;

    const graph: Graph = {
      id: 1,
      nodes: [
        node(0, 'test/node/output'),
        node(groupNodeId, '__internal/group/instance', { groupId }),
        node(9, 'test/node/input')
      ],
      edges: [
        edge(0, 0, groupNodeId, 'input_0'),
        edge(groupNodeId, 0, 9, 'value')
      ],
      groups: [{
        id: groupId,
        nodes: [
          node(3, '__internal/group/input'),
          node(1, 'test/node/output'), // B
          node(2, 'test/node/output'), // D
          node(4, '__internal/group/output')
        ],
        edges: [
          edge(3, 0, 1, 'input'), // inputBoundary → B
          edge(1, 0, 2, 'input'), // B → D (internal)
          edge(2, 0, 4, 'Out') // D → outputBoundary
        ],
        inputs: { input_0: { type: 'float' } },
        outputs: [{ type: 'float' }]
      }]
    };

    const result = expandGroups(graph);
    const remappedB = targetOf(result, 0);
    const remappedD = sourceOf(result, 9);

    expect(result.nodes.map(n => n.id)).not.toContain(groupNodeId);
    expect(result.nodes.map(n => n.id)).toContain(remappedB);
    expect(result.nodes.map(n => n.id)).toContain(remappedD);
    expect(remappedB).not.toBe(remappedD);

    expect(result.edges).toContainEqual(edge(0, 0, remappedB, 'input')); // A → B
    expect(result.edges).toContainEqual(edge(remappedB, 0, remappedD, 'input')); // B → D (internal)
    expect(result.edges).toContainEqual(edge(remappedD, 0, 9, 'value')); // D → C
    expect(result.edges.length).toBe(3);
  });

  it('expands a group with no external connections (isolated)', () => {
    const groupId = 20;
    const groupNodeId = 1;

    const graph: Graph = {
      id: 1,
      nodes: [node(groupNodeId, '__internal/group/instance', { groupId })],
      edges: [],
      groups: [{
        id: groupId,
        nodes: [
          node(3, '__internal/group/input'),
          node(2, 'test/node/output'),
          node(4, '__internal/group/output')
        ],
        edges: [
          edge(3, 0, 2, 'input'),
          edge(2, 0, 4, 'Out')
        ]
      }]
    };

    const result = expandGroups(graph);

    expect(result.nodes).toHaveLength(1);
    expect(result.nodes[0].id).not.toBe(groupNodeId);
    expect(result.nodes[0].type).toBe('test/node/output');
    expect(result.edges.length).toBe(0);
  });
  it('passes values set on the group node to unconnected group inputs', () => {
    const groupId = 5;
    const groupNodeId = 4;

    const graph: Graph = {
      id: 1,
      nodes: [
        node(1, 'test/node/output'),
        node(groupNodeId, '__internal/group/instance', { groupId, input_0: 3, input_1: 7 })
      ],
      edges: [edge(1, 0, groupNodeId, 'input_0')], // input_0 is connected, input_1 is not
      groups: [{
        id: groupId,
        nodes: [
          node(6, '__internal/group/input'),
          node(2, 'test/node/output', { input: 1 }),
          node(3, 'test/node/output', { input: 1 })
        ],
        edges: [
          edge(6, 0, 2, 'input'),
          edge(6, 1, 3, 'input')
        ],
        inputs: { input_0: { type: 'float' }, input_1: { type: 'float' } }
      }]
    };

    const result = expandGroups(graph);
    const remappedB = targetOf(result, 1);
    const remappedD = result.nodes.find(n => n.id !== 1 && n.id !== remappedB)!.id;

    expect(result.edges).toEqual([edge(1, 0, remappedB, 'input')]);
    expect(result.nodes.find(n => n.id === remappedB)?.props).toEqual({ input: 1 });
    expect(result.nodes.find(n => n.id === remappedD)?.props).toEqual({ input: 7 });
  });

  it('passes the group input default to unconnected inputs the group node has no value for', () => {
    const graph: Graph = {
      id: 1,
      nodes: [node(4, '__internal/group/instance', { groupId: 5 })],
      edges: [],
      groups: [{
        id: 5,
        nodes: [
          node(6, '__internal/group/input'),
          node(2, 'test/node/output', { input: 1 })
        ],
        edges: [edge(6, 0, 2, 'input')],
        inputs: { input_0: { type: 'float', value: 3 } }
      }]
    };

    const result = expandGroups(graph);

    expect(result.nodes[0].props).toEqual({ input: 3 });
  });

  it('keeps expanded IDs unique and safe integers when groups are nested deeply', () => {
    // Every group wraps the next one, the innermost one holds B:
    // A → [[[[B]]]] → C
    // A → [[[[B]]]] → D
    const depth = 4;
    const groups: Graph['groups'] = [];
    for (let level = 0; level < depth; level++) {
      const innermost = level === depth - 1;
      groups.push({
        id: 100 + level,
        nodes: [
          node(1, '__internal/group/input'),
          innermost
            ? node(2, 'test/node/output')
            : node(2, '__internal/group/instance', { groupId: 100 + level + 1 }),
          node(3, '__internal/group/output')
        ],
        edges: [
          edge(1, 0, 2, innermost ? 'input' : 'input_0'),
          edge(2, 0, 3, 'out_0')
        ],
        inputs: { input_0: { type: 'float' } },
        outputs: [{ type: 'float' }]
      });
    }

    const graph: Graph = {
      id: 1,
      nodes: [
        node(10, 'test/node/output'),
        node(11, '__internal/group/instance', { groupId: 100 }),
        node(12, 'test/node/input'),
        node(13, '__internal/group/instance', { groupId: 100 }),
        node(14, 'test/node/input')
      ],
      edges: [
        edge(10, 0, 11, 'input_0'),
        edge(11, 0, 12, 'value'),
        edge(10, 0, 13, 'input_0'),
        edge(13, 0, 14, 'value')
      ],
      groups
    };

    const result = expandGroups(graph);

    const ids = result.nodes.map(n => n.id);
    expect(ids).toHaveLength(5);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.every(id => Number.isSafeInteger(id))).toBe(true);

    const bForC = sourceOf(result, 12);
    const bForD = sourceOf(result, 14);
    expect(bForC).not.toBe(bForD);
    expect(result.edges).toHaveLength(4);
    expect(result.edges).toContainEqual(edge(10, 0, bForC, 'input'));
    expect(result.edges).toContainEqual(edge(10, 0, bForD, 'input'));

    // The runtime cache is keyed by node ID, so the IDs must not change between runs
    expect(expandGroups(graph).nodes.map(n => n.id)).toEqual(ids);
  });
});

describe('MemoryRuntimeExecutor', () => {
  it('runs an output node that has nothing connected', async () => {
    let received: NodeValue[] = [];
    const output = {
      id: 'test/node/output',
      inputs: { input: { type: 'path', accepts: ['geometry'], external: true } },
      outputs: [],
      execute: (inputs: NodeValue[]) => {
        received = inputs;
        return inputs[0];
      }
    } as unknown as NodeDefinition;
    const runtime = new MemoryRuntimeExecutor(createMockNodeRegistry([output]));

    const graph: Graph = { id: 0, nodes: [node(0, output.id)], edges: [], groups: [] };
    const result = await runtime.execute(graph, {});

    // an empty nested array, the viewer shows no geometry for it
    expect([...received[0] as Int32Array]).toEqual([0, 1, 1, 1]);
    expect([...result]).toEqual([0, 1, 1, 1]);
  });
});
