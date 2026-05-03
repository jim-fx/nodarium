import { describe, expect, it } from 'vitest';
import { expandGroups } from './runtime-executor';
import type { Graph } from '@nodarium/types';

// Helpers to build minimal serialized nodes/edges
function node(id: number, type: string, props?: Record<string, number>) {
  return { id, type: type as Graph['nodes'][0]['type'], position: [0, 0] as [number, number], ...(props ? { props } : {}) };
}

function edge(from: number, fromSocket: number, to: number, toSocket: string): [number, number, number, string] {
  return [from, fromSocket, to, toSocket];
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
    const remappedB = (groupNodeId + 1) * 1_000_000 + 2; // 5_000_002

    const graph: Graph = {
      id: 1,
      nodes: [
        node(1, 'test/node/output'),
        node(groupNodeId, '__internal/group/instance', { groupId }),
        node(3, 'test/node/input')
      ],
      edges: [
        edge(1, 0, groupNodeId, 'input_0'),  // A → group
        edge(groupNodeId, 0, 3, 'value')     // group → C
      ],
      groups: [{
        id: groupId,
        nodes: [
          node(6, '__internal/group/input'),
          node(2, 'test/node/output'),
          node(7, '__internal/group/output')
        ],
        edges: [
          edge(6, 0, 2, 'input'),  // inputBoundary → B
          edge(2, 0, 7, 'Out')     // B → outputBoundary
        ],
        inputs: { input_0: { type: 'float' } },
        outputs: [{ type: 'float', label: 'Output 0' }]
      }]
    };

    const result = expandGroups(graph);

    const ids = result.nodes.map(n => n.id);
    expect(ids).not.toContain(groupNodeId);
    expect(ids).toContain(remappedB);
    expect(ids).toContain(1); // A
    expect(ids).toContain(3); // C
    expect(result.nodes.length).toBe(3); // A, B(remapped), C

    expect(result.edges).toContainEqual(edge(1, 0, remappedB, 'input'));   // A → B
    expect(result.edges).toContainEqual(edge(remappedB, 0, 3, 'value')); // B → C
    expect(result.edges.length).toBe(2);
  });

  it('expands a group with two internal nodes (B→D) and preserves the internal edge', () => {
    // A → [B → D] → C
    const groupId = 10;
    const groupNodeId = 5;
    const remappedB = (groupNodeId + 1) * 1_000_000 + 1; // 6_000_001
    const remappedD = (groupNodeId + 1) * 1_000_000 + 2; // 6_000_002

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
          node(1, 'test/node/output'),  // B
          node(2, 'test/node/output'),  // D
          node(4, '__internal/group/output')
        ],
        edges: [
          edge(3, 0, 1, 'input'),  // inputBoundary → B
          edge(1, 0, 2, 'input'),  // B → D (internal)
          edge(2, 0, 4, 'Out')     // D → outputBoundary
        ],
        inputs: { input_0: { type: 'float' } },
        outputs: [{ type: 'float' }]
      }]
    };

    const result = expandGroups(graph);

    expect(result.nodes.map(n => n.id)).not.toContain(groupNodeId);
    expect(result.nodes.map(n => n.id)).toContain(remappedB);
    expect(result.nodes.map(n => n.id)).toContain(remappedD);

    expect(result.edges).toContainEqual(edge(0, 0, remappedB, 'input'));    // A → B
    expect(result.edges).toContainEqual(edge(remappedB, 0, remappedD, 'input')); // B → D (internal)
    expect(result.edges).toContainEqual(edge(remappedD, 0, 9, 'value'));    // D → C
    expect(result.edges.length).toBe(3);
  });

  it('expands a group with no external connections (isolated)', () => {
    const groupId = 20;
    const groupNodeId = 1;
    const remappedB = (groupNodeId + 1) * 1_000_000 + 2; // 2_000_002

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

    expect(result.nodes.map(n => n.id)).not.toContain(groupNodeId);
    expect(result.nodes.map(n => n.id)).toContain(remappedB);
    expect(result.edges.length).toBe(0);
  });
});
