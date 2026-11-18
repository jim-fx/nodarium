import type { Graph, GroupDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { expandGroups, expandWithChains } from './expand-groups';

function node(id: number, type: string, props?: Record<string, number>) {
  return {
    id,
    type: type as Graph['nodes'][0]['type'],
    position: [0, 0] as [number, number],
    ...(props ? { props } : {})
  };
}

const instance = (id: number, groupId: number) =>
  node(id, '__internal/group/instance', { groupId });

/** A group whose input runs through `inner` into its output */
function group(id: number, inner: ReturnType<typeof node>): GroupDefinition {
  return {
    id,
    nodes: [node(100, '__internal/group/input'), inner, node(101, '__internal/group/output')],
    edges: [[100, 0, inner.id, 'input'], [inner.id, 0, 101, 'Out']],
    inputs: { input_0: { type: 'float' } },
    outputs: [{ type: 'float', label: 'Output 0' }]
  };
}

/**
 * 1 → group node 2 (group 10: node 20) → group node 3 (group 11: group node 30 of group 10)
 * → 4, so node 20 runs three times: in 2 and in 30 inside 3
 */
const graph: Graph = {
  id: 1,
  nodes: [
    node(1, 'test/node/output'),
    instance(2, 10),
    instance(3, 11),
    node(4, 'test/node/input')
  ],
  edges: [[1, 0, 2, 'input_0'], [2, 0, 3, 'input_0'], [3, 0, 4, 'value']],
  groups: [group(10, node(20, 'test/node/output')), group(11, instance(30, 10))]
};

describe('expandWithChains', () => {
  it('runs the graph that expandGroups gives', () => {
    const { graph: expanded } = expandWithChains(graph);
    expect(expanded).toEqual(expandGroups(graph));
  });

  it('leads to every node that runs through the group nodes it is inside of', () => {
    const { graph: expanded, chains } = expandWithChains(graph);
    expect(chains.size).toBe(expanded.nodes.length);
    expect(chains.get(1)).toEqual([1]);
    expect(chains.get(4)).toEqual([4]);

    const inner = expanded.nodes.filter((n) => chains.get(n.id)!.length > 1);
    expect(inner.map((n) => chains.get(n.id)).sort()).toEqual([
      [2, 20],
      [3, 30, 20]
    ]);
  });

  it('tells two nodes of the same group apart by the group node they run in', () => {
    const twice: Graph = {
      ...graph,
      nodes: [node(1, 'test/node/output'), instance(2, 10), instance(5, 10)],
      edges: [[1, 0, 2, 'input_0'], [1, 0, 5, 'input_0']],
      groups: [group(10, node(20, 'test/node/output'))]
    };
    const { graph: expanded, chains } = expandWithChains(twice);
    const ran = expanded.nodes.filter((n) => n.id !== 1).map((n) => chains.get(n.id));
    expect(ran).toEqual(expect.arrayContaining([[2, 20], [5, 20]]));
    expect(ran).toHaveLength(2);
  });

  it('is the graph itself without groups', () => {
    const flat: Graph = { id: 1, nodes: [node(1, 'test/node/output')], edges: [], groups: [] };
    const { graph: expanded, chains } = expandWithChains(flat);
    expect(expanded).toBe(flat);
    expect([...chains]).toEqual([[1, [1]]]);
  });
});
