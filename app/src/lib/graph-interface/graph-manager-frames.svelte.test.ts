import { CURRENT_GRAPH_VERSION } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import { GROUP_INSTANCE } from '@nodarium/types';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { createMockNodeRegistry, mockFloatInputNode, mockFloatOutputNode } from './test-utils';

function framedGraph(): Graph {
  return {
    id: 1,
    version: CURRENT_GRAPH_VERSION,
    settings: {},
    nodes: [
      { id: 0, type: 'test/node/input', position: [0, 0], props: {} },
      { id: 1, type: 'test/node/output', position: [30, 0], props: {} },
      { id: 2, type: 'test/node/output', position: [60, 0], props: {} },
      { id: 3, type: GROUP_INSTANCE, position: [90, 0], props: { groupId: 10 } }
    ],
    edges: [[0, 0, 1, 'input'], [1, 0, 2, 'input']],
    groups: [{
      id: 10,
      name: 'Inner',
      nodes: [{ id: 11, type: 'test/node/input', position: [0, 0], props: {} }],
      edges: [],
      frames: [{ id: 1, label: 'Inside', nodes: [11] }]
    }],
    frames: [
      { id: 1, label: 'Values', color: 'green', nodes: [0, 1] },
      { id: 2, label: 'Last', nodes: [2] }
    ]
  };
}

async function load(graph = framedGraph()) {
  const manager = new GraphManager(
    createMockNodeRegistry([mockFloatInputNode, mockFloatOutputNode])
  );
  await manager.load(graph);
  return manager;
}

describe('frames in the graph file', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('come back from load() as they were', async () => {
    const graph = framedGraph();
    const manager = await load(structuredClone(graph));
    const serialized = manager.serialize();
    expect(serialized.frames).toEqual(graph.frames);
    expect(serialized.groups[0].frames).toEqual(graph.groups[0].frames);
  });

  it('are left out of a graph without them', async () => {
    const graph = framedGraph();
    delete graph.frames;
    delete graph.groups[0].frames;
    const serialized = (await load(graph)).serialize();
    expect('frames' in serialized).toBe(false);
    expect('frames' in serialized.groups[0]).toBe(false);
  });

  it('stay with their level when a group is entered and left', async () => {
    const manager = await load();
    manager.enterGroup(3);
    expect(manager.frames).toEqual([{ id: 1, label: 'Inside', nodes: [11] }]);
    expect(manager.serialize().frames).toEqual(framedGraph().frames);
    manager.exitGroup();
    expect(manager.frames).toEqual(framedGraph().frames);
    expect(manager.serialize().groups[0].frames).toEqual(framedGraph().groups[0].frames);
  });

  it('drop the nodes that are gone, and a frame without nodes, when the graph is loaded', async () => {
    const graph = framedGraph();
    graph.frames = [
      { id: 1, label: 'Values', nodes: [0, 99, 1] },
      { id: 2, label: 'Again', nodes: [1, 98] }
    ];
    const manager = await load(graph);
    expect(manager.frames).toEqual([{ id: 1, label: 'Values', nodes: [0, 1] }]);
  });

  it('let go of a removed node, and the last one takes its frame with it, undo brings both back', async () => {
    const manager = await load();
    manager.removeNode(manager.getNode(1)!);
    expect(manager.serialize().frames).toEqual([
      { id: 1, label: 'Values', color: 'green', nodes: [0] },
      { id: 2, label: 'Last', nodes: [2] }
    ]);
    manager.removeNode(manager.getNode(2)!);
    expect(manager.serialize().frames).toEqual([
      { id: 1, label: 'Values', color: 'green', nodes: [0] }
    ]);
    vi.runAllTimers();
    manager.undo();
    manager.undo();
    expect(manager.serialize().frames).toEqual(framedGraph().frames);
  });
});
