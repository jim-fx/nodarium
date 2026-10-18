import type { NodeDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { layoutGraph } from './layoutGraph';
import { getInputSocketOffset, getNodeHeight, getOutputSocketOffset } from './nodeHelpers';

const definitions = Object.fromEntries(
  [
    { id: 'layout/output', inputs: { input: { type: 'path' } } },
    { id: 'layout/stem', inputs: { length: { type: 'float' } } },
    { id: 'layout/branch', inputs: { plant: { type: 'path' }, length: { type: 'float' } } },
    { id: 'layout/random', inputs: { min: { type: 'float' }, max: { type: 'float' } } }
  ].map((d) => [d.id, d as unknown as NodeDefinition])
);

function node(id: number, type: string, position: [number, number] = [0, 0]): SerializedNode {
  return { id, type: `layout/${type}` as SerializedNode['type'], position };
}

function layout(nodes: SerializedNode[], edges: SerializedEdge[]) {
  return layoutGraph(nodes, edges, (n) => definitions[n.type]);
}

describe('layoutGraph', () => {
  it('returns nothing for an empty graph', () => {
    expect(layout([], []).size).toBe(0);
  });

  it('puts every node one column left of the nodes it feeds, the output stays', () => {
    const nodes = [
      node(1, 'output', [300, 80]),
      node(2, 'branch', [17, 3]),
      node(3, 'stem', [-50, 200]),
      node(4, 'random', [90, 90])
    ];
    const edges: SerializedEdge[] = [
      [3, 0, 2, 'plant'],
      [2, 0, 1, 'input'],
      [4, 0, 2, 'length']
    ];
    const positions = layout(nodes, edges);

    expect(positions.get(1)).toEqual([300, 80]);
    expect(positions.get(2)![0]).toBe(270);
    expect(positions.get(3)![0]).toBe(240);
    expect(positions.get(4)![0]).toBe(240);
  });

  it('lines up a chain of nodes at the top, its edges drop by the same amount', () => {
    const nodes = [node(1, 'output', [0, 0]), node(2, 'branch'), node(3, 'stem')];
    const edges: SerializedEdge[] = [
      [3, 0, 2, 'plant'],
      [2, 0, 1, 'input']
    ];
    const positions = layout(nodes, edges);

    expect(positions.get(1)![1]).toBe(0);
    expect(positions.get(2)![1]).toBe(0);
    expect(positions.get(3)![1]).toBe(0);
    for (const [from, output, to, input] of edges) {
      const fromNode = nodes.find((n) => n.id === from)!;
      const toNode = nodes.find((n) => n.id === to)!;
      expect(
        positions.get(to)![1] + getInputSocketOffset(definitions[toNode.type], input)
          - positions.get(from)![1] - getOutputSocketOffset(fromNode.type, output)
      ).toBe(5);
    }
  });

  it('stacks the nodes of a column without overlap', () => {
    const nodes = [
      node(1, 'output'),
      node(2, 'branch'),
      node(3, 'stem'),
      node(4, 'random'),
      node(5, 'random')
    ];
    const edges: SerializedEdge[] = [
      [2, 0, 1, 'input'],
      [3, 0, 2, 'plant'],
      [4, 0, 2, 'length'],
      [5, 0, 3, 'length']
    ];
    const positions = layout(nodes, edges);

    const column = [3, 4]
      .map((id) => ({ y: positions.get(id)![1], node: nodes.find((n) => n.id === id)! }))
      .sort((a, b) => a.y - b.y);
    expect(column[0].node.id).toBe(3);
    expect(column[1].y).toBeGreaterThanOrEqual(
      column[0].y + getNodeHeight(definitions[column[0].node.type])
    );
  });

  it('puts a node that feeds two columns left of both', () => {
    const nodes = [node(1, 'output'), node(2, 'branch'), node(3, 'branch'), node(4, 'random')];
    const edges: SerializedEdge[] = [
      [3, 0, 2, 'plant'],
      [2, 0, 1, 'input'],
      [4, 0, 2, 'length'],
      [4, 0, 3, 'length']
    ];
    const positions = layout(nodes, edges);

    expect(positions.get(4)![0]).toBeLessThan(positions.get(3)![0]);
  });

  it('survives a cycle', () => {
    const nodes = [node(1, 'branch'), node(2, 'branch')];
    const edges: SerializedEdge[] = [
      [1, 0, 2, 'plant'],
      [2, 0, 1, 'plant']
    ];
    expect(layout(nodes, edges).size).toBe(2);
  });
});
