import { CURRENT_GRAPH_VERSION } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import { GROUP_INSTANCE } from '@nodarium/types';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { GraphState } from './graph-state.svelte';
import {
  createGraphOptions,
  createMockNodeRegistry,
  mockFloatInputNode,
  mockFloatOutputNode
} from './test-utils';

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

function editor() {
  const manager = new GraphManager(
    createMockNodeRegistry([mockFloatInputNode, mockFloatOutputNode])
  );
  const state = new GraphState(manager, () => createGraphOptions());
  const [a, b, c, d] = [0, 30, 60, 90].map((x) =>
    manager.createNode({ type: 'test/node/output', position: [x, 0], props: {} })!
  );
  manager.createEdge(a, 0, b, 'input');
  manager.createEdge(b, 0, c, 'input');
  manager.createEdge(c, 0, d, 'input');
  return { manager, state, ids: [a.id, b.id, c.id, d.id] };
}

describe('frames in the editor', () => {
  it('frame the selection, which selects the frame alone', () => {
    const { manager, state, ids } = editor();
    state.activeNodeId = ids[0];
    state.selectedNodes.add(ids[1]);
    state.frameSelection();
    expect(manager.serialize().frames).toEqual([
      { id: 1, label: 'Frame', nodes: [ids[0], ids[1]] }
    ]);
    expect(state.activeFrameId).toBe(1);
    expect(state.activeNodeId).toBe(-1);
    expect(state.selectedNodes.size).toBe(0);
    expect(state.selectedNodeIds()).toEqual([ids[0], ids[1]]);

    manager.renameFrame(1, 'Kept');
    state.frameSelection();
    expect(manager.frames.map((f) => [f.id, f.label])).toEqual([[1, 'Kept']]);
  });

  it('take nodes from the frame they were in, Alt+P takes them out', () => {
    const { manager, ids } = editor();
    manager.createFrame([ids[0], ids[1], ids[2]]);
    manager.createFrame([ids[2], ids[3]], 'Second');
    expect(manager.frames.map((f) => [f.id, f.nodes])).toEqual([
      [1, [ids[0], ids[1]]],
      [2, [ids[2], ids[3]]]
    ]);
    manager.removeFromFrames([ids[0], ids[1]]);
    expect(manager.frames.map((f) => f.id)).toEqual([2]);
  });

  it('are renamed, coloured and removed without their nodes, each one undo step', () => {
    const { manager, ids } = editor();
    const frame = manager.createFrame([ids[0], ids[1]])!;
    manager.renameFrame(frame.id, 'Stem');
    manager.setFrameColor(frame.id, 'brown');
    expect(manager.serialize().frames).toEqual([
      { id: 1, label: 'Stem', color: 'brown', nodes: [ids[0], ids[1]] }
    ]);
    manager.setFrameColor(frame.id, 'gray');
    expect(manager.serialize().frames?.[0].color).toBeUndefined();
    manager.removeFrame(frame.id);
    expect(manager.serialize().frames).toBeUndefined();
    expect(manager.nodes.size).toBe(4);

    manager.undo();
    expect(manager.frames[0].color).toBeUndefined();
    manager.undo();
    expect(manager.frames[0].color).toBe('brown');
    manager.undo();
    expect([manager.frames[0].label, manager.frames[0].color]).toEqual(['Stem', undefined]);
    manager.undo();
    expect(manager.frames[0].label).toBe('Frame');
    manager.undo();
    expect(manager.frames).toEqual([]);
    manager.redo();
    expect(manager.frames[0].nodes).toEqual([ids[0], ids[1]]);
  });

  it('go when the last of their nodes is deleted', () => {
    const { manager, ids } = editor();
    manager.createFrame([ids[0], ids[1]]);
    manager.transaction(() => {
      manager.removeNode(manager.getNode(ids[0])!);
      manager.removeNode(manager.getNode(ids[1])!);
    });
    expect(manager.frames).toEqual([]);
    const node = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} })!;
    expect(manager.frameOf(node.id)).toBeUndefined();
  });

  it('are copied when all their nodes are, and pasted with new ids', () => {
    const { manager, state, ids } = editor();
    manager.createFrame([ids[0], ids[1]], 'Copied');
    manager.createFrame([ids[2], ids[3]], 'Half');
    state.activeNodeId = ids[0];
    state.selectedNodes.add(ids[1]);
    state.selectedNodes.add(ids[2]);
    state.copyNodes();
    state.pasteNodes();
    const pasted = [...state.selectedNodes];
    expect(pasted).toHaveLength(3);
    expect(manager.frames.map((f) => f.label)).toEqual(['Copied', 'Half', 'Copied']);
    const copy = manager.frames[2];
    expect(copy.id).toBe(3);
    expect(copy.nodes).toHaveLength(2);
    expect(copy.nodes.every((id) => pasted.includes(id))).toBe(true);
  });

  it('copy the nodes of a selected frame along with it', () => {
    const { manager, state, ids } = editor();
    const frame = manager.createFrame([ids[0], ids[1]])!;
    state.selectFrame(frame.id);
    state.copyNodes();
    expect(state.clipboard?.nodes.map((n) => n.id)).toEqual([ids[0], ids[1]]);
    expect(state.clipboard?.frames).toHaveLength(1);
  });

  it('keep their other nodes when some are grouped, and hold the group node of their nodes', () => {
    const { manager, ids } = editor();
    manager.createFrame([ids[0], ids[1], ids[2]], 'Part');
    manager.createFrame([ids[3]], 'Other');
    const groupNode = manager.groupNodes([ids[1], ids[2]])!;
    expect(manager.frames.map((f) => [f.label, f.nodes])).toEqual([
      ['Part', [ids[0], groupNode.id]],
      ['Other', [ids[3]]]
    ]);
    expect(manager.serialize().groups[0].frames).toBeUndefined();

    manager.ungroupNode(groupNode.id);
    const part = manager.frames.find((f) => f.label === 'Part')!;
    expect(part.nodes).toHaveLength(3);
    expect(part.nodes).toContain(ids[0]);
    expect(part.nodes).not.toContain(groupNode.id);
  });

  it('let go of nodes grouped together with nodes of another frame', () => {
    const { manager, ids } = editor();
    manager.createFrame([ids[0], ids[1]], 'A');
    manager.createFrame([ids[2], ids[3]], 'B');
    const groupNode = manager.groupNodes([ids[1], ids[2]])!;
    expect(manager.frames.map((f) => f.nodes)).toEqual([[ids[0]], [ids[3]]]);
    expect(manager.frameOf(groupNode.id)).toBeUndefined();
  });

  it('follow their nodes when they are formatted', () => {
    const { manager, ids } = editor();
    manager.createFrame([ids[0], ids[1]]);
    manager.formatNodes();
    expect(manager.frames[0].nodes).toEqual([ids[0], ids[1]]);
  });
});
