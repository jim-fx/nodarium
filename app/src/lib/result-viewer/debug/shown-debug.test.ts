import { DEBUG_NODE, type Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { debugLinks, debugOrder, selectedDebugSource, shownDebugSource } from './shown-debug';

const node = (id: number, type: string, props?: Record<string, number>) =>
  ({ id, type, position: [0, 0], ...(props ? { props } : {}) }) as Graph['nodes'][number];

// a leaf (2) watched by debug node 10, its amount comes from a float (4), a stem (1) watched by
// debug node 11
const graph = {
  id: 1,
  nodes: [
    node(1, 'max/plantarium/stem'),
    node(2, 'max/plantarium/leaf'),
    node(3, 'max/core/output'),
    node(4, 'max/core/float'),
    node(10, DEBUG_NODE),
    node(11, DEBUG_NODE)
  ],
  edges: [[1, 0, 3, 'input'], [4, 0, 10, 'amount'], [2, 0, 10, 'input'], [1, 0, 11, 'input']],
  groups: []
} as Graph;

// group node 7 runs group 50, where debug node 61 watches the leaf 60 that the group gives out.
// Debug node 11 watches the group node from outside.
const grouped = {
  id: 2,
  nodes: [
    node(7, '__internal/group/instance', { groupId: 50 }),
    node(3, 'max/core/output'),
    node(11, DEBUG_NODE)
  ],
  edges: [[7, 0, 3, 'input'], [7, 0, 11, 'input']],
  groups: [{
    id: 50,
    nodes: [
      node(60, 'max/plantarium/leaf'),
      node(61, DEBUG_NODE),
      node(62, '__internal/group/output')
    ],
    edges: [[60, 0, 62, 'Out'], [60, 0, 61, 'input']],
    inputs: {},
    outputs: [{ type: 'geometry', label: 'Leaf' }]
  }]
} as unknown as Graph;

describe('debugLinks', () => {
  it('finds the node every debug node shows, not the ones of its amount and clip', () => {
    expect(debugLinks(graph).map(({ debugNode, source }) => [debugNode, source])).toEqual([
      [10, 2],
      [11, 1]
    ]);
  });

  it('finds the debug nodes inside groups, with the group nodes they run in', () => {
    const links = debugLinks(grouped);
    expect(links.map((l) => [l.debugChain, l.sourceChain])).toEqual(
      expect.arrayContaining([[[11], [7, 60]], [[7, 61], [7, 60]]])
    );
  });
});

describe('debugOrder', () => {
  it('keeps the order the nodes were debugged in, new ones last', () => {
    expect(debugOrder([], [2, 1])).toEqual([2, 1]);
    expect(debugOrder([5, 2], [1, 2])).toEqual([2, 1]);
    expect(debugOrder([2, 1], [1, 2, 4])).toEqual([2, 1, 4]);
  });
});

describe('the debug node the viewer shows', () => {
  const links = debugLinks(graph);
  const order = [2, 1];

  it('is the one of the selected debug node', () => {
    expect(shownDebugSource(links, order, 10)).toBe(2);
    expect(shownDebugSource(links, order, 11)).toBe(1);
  });

  it('is the one connected to the selected node', () => {
    expect(shownDebugSource(links, order, 2)).toBe(2);
  });

  it('is the one debugged last without a selection that has one', () => {
    expect(shownDebugSource(links, order, undefined)).toBe(1);
    expect(shownDebugSource(links, order, 3)).toBe(1);
    expect(shownDebugSource(links, [], 3)).toBeUndefined();
  });

  it('leads the local view to the debug node only for a selection that has one', () => {
    expect(selectedDebugSource(links, order, 10)).toBe(2);
    expect(selectedDebugSource(links, order, 1)).toBe(1);
    expect(selectedDebugSource(links, order, 3)).toBeUndefined();
    // a debug node whose node gave nothing, e.g. after an error
    expect(selectedDebugSource(links, [1], 10)).toBeUndefined();
  });

  it('is found inside a group by the ids the editor shows there', () => {
    const links = debugLinks(grouped);
    const leaf = links[0].source;
    // both debug nodes show the leaf as it runs inside group node 7
    expect(links.every((l) => l.source === leaf)).toBe(true);
    const order = [leaf];
    // outside: the group node and the debug node next to it
    expect(selectedDebugSource(links, order, 7)).toBe(leaf);
    expect(selectedDebugSource(links, order, 11)).toBe(leaf);
    // inside group node 7: the debug node and the leaf there, not the ids outside
    expect(selectedDebugSource(links, order, 61, [7])).toBe(leaf);
    expect(selectedDebugSource(links, order, 60, [7])).toBe(leaf);
    expect(selectedDebugSource(links, order, 11, [7])).toBeUndefined();
    expect(selectedDebugSource(links, order, 61)).toBeUndefined();
  });
});
