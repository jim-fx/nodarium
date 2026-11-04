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

  it('keeps a wide fan-in roughly centred on the output node', () => {
    const nodes = [
      node(1, 'output', [0, 0]),
      node(2, 'branch'),
      node(3, 'branch'),
      node(4, 'branch'),
      node(5, 'branch'),
      node(6, 'branch'),
      node(7, 'stem'),
      node(8, 'stem'),
      node(9, 'stem'),
      node(10, 'stem'),
      node(11, 'stem')
    ];
    const edges: SerializedEdge[] = [];
    for (const id of [2, 3, 4, 5, 6]) {
      edges.push([id, 0, 1, 'input']);
      edges.push([id + 5, 0, id, 'plant']);
    }
    const positions = layout(nodes, edges);

    const top = (id: number) => positions.get(id)![1];
    const minY = Math.min(...nodes.map((n) => top(n.id)));
    const maxY = Math.max(...nodes.map((n) => top(n.id) + getNodeHeight(definitions[n.type])));
    expect(minY).toBeLessThan(0);
    expect(Math.abs((minY + maxY) / 2)).toBeLessThan(30);
  });

  it('never overlaps two nodes of a column, whatever the graph', () => {
    let seed = 1;
    const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    const types = ['branch', 'stem', 'random'];
    const inputs: Record<string, string[]> = {
      branch: ['plant', 'length'],
      stem: ['length'],
      random: ['min', 'max']
    };

    for (let run = 0; run < 200; run++) {
      const nodes = [node(1, 'output', [0, 0])];
      const edges: SerializedEdge[] = [];
      const count = 2 + Math.floor(random() * 25);
      for (let id = 2; id <= count; id++) {
        nodes.push(node(id, types[Math.floor(random() * 3)], [0, random() * 100]));
        // feed a free input of an earlier node, so the graph stays acyclic
        const target = nodes[Math.floor(random() * (nodes.length - 1))];
        const keys = inputs[target.type.slice('layout/'.length)] ?? ['input'];
        const key = keys[Math.floor(random() * keys.length)];
        edges.push([id, 0, target.id, key]);
      }
      const positions = layout(nodes, edges);

      const byX = new Map<number, { y: number; h: number }[]>();
      for (const n of nodes) {
        const [x, y] = positions.get(n.id)!;
        if (!byX.has(x)) byX.set(x, []);
        byX.get(x)!.push({ y, h: getNodeHeight(definitions[n.type]) });
      }
      for (const column of byX.values()) {
        column.sort((a, b) => a.y - b.y);
        for (let i = 1; i < column.length; i++) {
          expect(column[i].y).toBeGreaterThanOrEqual(column[i - 1].y + column[i - 1].h + 5 - 1e-9);
        }
      }
    }
  });

  it('puts more space between two sub groups than between siblings', () => {
    const nodes = [
      node(1, 'output', [0, 0]),
      node(2, 'branch'),
      node(3, 'branch'),
      node(4, 'stem'),
      node(5, 'stem'),
      node(6, 'random'),
      node(7, 'random'),
      node(8, 'random'),
      node(9, 'random')
    ];
    // output <- 2 <- (4 <- 6, 7) and output <- 3 <- (5 <- 8, 9)
    const edges: SerializedEdge[] = [
      [2, 0, 1, 'input'],
      [3, 0, 1, 'input'],
      [4, 0, 2, 'plant'],
      [5, 0, 3, 'plant'],
      [6, 0, 4, 'length'],
      [7, 0, 4, 'length'],
      [8, 0, 5, 'length'],
      [9, 0, 5, 'length']
    ];
    const positions = layout(nodes, edges);
    const height = getNodeHeight(definitions['layout/random']);
    const space = (a: number, b: number) => positions.get(b)![1] - positions.get(a)![1] - height;

    const ids = [6, 7, 8, 9].sort((a, b) => positions.get(a)![1] - positions.get(b)![1]);
    expect(space(ids[0], ids[1])).toBeCloseTo(5);
    expect(space(ids[2], ids[3])).toBeCloseTo(5);
    expect(space(ids[1], ids[2])).toBeGreaterThan(space(ids[0], ids[1]));
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
