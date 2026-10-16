import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { hashGraph } from './graph-hash';

function createGraph(): Graph {
  return {
    id: 0,
    meta: { title: 'Fern', lastModified: '2026-10-16T10:00:00Z' },
    settings: { randomSeed: false },
    nodes: [
      { id: 0, type: 'max/plantarium/output', position: [0, 0] },
      { id: 1, type: 'max/plantarium/stem', props: { length: 4 }, position: [-20, 0] }
    ],
    edges: [[1, 0, 0, 'plant']],
    groups: []
  };
}

describe('hashGraph', () => {
  it('ignores node positions and meta data', () => {
    const graph = createGraph();
    const moved = createGraph();
    moved.nodes[1].position = [100, 50];
    moved.meta = { title: 'Renamed', lastModified: '2026-10-17T10:00:00Z' };
    expect(hashGraph(moved)).toBe(hashGraph(graph));
  });

  it('changes when a prop changes', () => {
    const changed = createGraph();
    changed.nodes[1].props = { length: 5 };
    expect(hashGraph(changed)).not.toBe(hashGraph(createGraph()));
  });

  it('changes when the settings change', () => {
    const changed = createGraph();
    changed.settings = { randomSeed: true };
    expect(hashGraph(changed)).not.toBe(hashGraph(createGraph()));
  });

  it('changes when an edge is removed', () => {
    const changed = createGraph();
    changed.edges = [];
    expect(hashGraph(changed)).not.toBe(hashGraph(createGraph()));
  });
});
