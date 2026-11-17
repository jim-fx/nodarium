import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { placeNodes } from './place-nodes';

function graph(nodes: Graph['nodes'], edges: Graph['edges'] = []): Graph {
  return { id: 0, nodes, edges, groups: [] };
}

describe('placeNodes', () => {
  it('returns a graph whose nodes all have a position as it is', () => {
    const placed = graph([{ id: 1, type: 'a/b/output', position: [1, 2] }]);
    expect(placeNodes(placed)).toBe(placed);
  });

  it('puts a node one column left of the leftmost node it feeds', () => {
    // 1 feeds 2 and 3, 2 feeds 3: 1 is two columns left of the output
    const result = placeNodes(
      graph(
        [
          { id: 1, type: 'a/b/stem' },
          { id: 2, type: 'a/b/noise' },
          { id: 3, type: 'a/b/output' }
        ],
        [[1, 0, 2, 'plant'], [2, 0, 3, 'input'], [1, 0, 3, 'input']]
      )
    );
    expect(result.nodes.map((node) => node.position)).toEqual([[-60, 0], [-30, 0], [0, 0]]);
  });

  it('stacks the nodes of a column by their height, from the output down', () => {
    const result = placeNodes(
      graph(
        [
          { id: 1, type: 'a/b/float' },
          { id: 2, type: 'a/b/float' },
          { id: 3, type: 'a/b/output', position: [100, 50] }
        ],
        [[1, 0, 3, 'a'], [2, 0, 3, 'b']]
      ),
      { height: () => 20 }
    );
    expect(result.nodes.map((node) => node.position)).toEqual([[70, 50], [70, 75], [100, 50]]);
  });

  it('places the nodes of the groups and leaves the graph it was given alone', () => {
    const input = graph([{ id: 1, type: 'a/b/output', position: [0, 0] }]);
    input.groups = [{ id: 5, nodes: [{ id: 1, type: 'a/b/float' }], edges: [] }];
    const result = placeNodes(input);
    expect(result.groups[0].nodes[0].position).toEqual([0, 0]);
    expect(input.groups[0].nodes[0].position).toBeUndefined();
  });

  it('survives a cycle', () => {
    const result = placeNodes(
      graph([{ id: 1, type: 'a/b/x' }, { id: 2, type: 'a/b/y' }], [[1, 0, 2, 'a'], [2, 0, 1, 'a']])
    );
    expect(result.nodes.every((node) => node.position)).toBe(true);
  });
});
