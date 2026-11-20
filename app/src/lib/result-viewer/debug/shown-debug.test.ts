import { DEBUG_NODE } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { debugLinks, debugOrder, selectedDebugSource, shownDebugSource } from './shown-debug';

// a leaf (2) watched by debug node 10, a stem (1) watched by debug node 11
const graph = {
  nodes: [
    { id: 1, type: 'max/plantarium/stem' },
    { id: 2, type: 'max/plantarium/leaf' },
    { id: 3, type: 'max/core/output' },
    { id: 10, type: DEBUG_NODE },
    { id: 11, type: DEBUG_NODE }
  ],
  edges: [[1, 0, 3, 'input'], [2, 0, 10, 'input'], [1, 0, 11, 'input']]
} as Parameters<typeof debugLinks>[0];

describe('debugLinks', () => {
  it('finds the node every debug node shows', () => {
    expect(debugLinks(graph)).toEqual([
      { debugNode: 10, source: 2 },
      { debugNode: 11, source: 1 }
    ]);
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
});
