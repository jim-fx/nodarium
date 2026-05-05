import { assert, describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { GraphState } from './graph-state.svelte';
import { createMockNodeRegistry, mockFloatInputNode, mockFloatOutputNode } from './test-utils';

// GraphState constructor reads localStorage synchronously — mock before any instantiation
Object.defineProperty(globalThis, 'localStorage', {
  value: {
    getItem: () => null,
    setItem: () => {},
    removeItem: () => {},
    clear: () => {},
    length: 0,
    key: () => null
  } as Storage,
  writable: true,
  configurable: true
});

function createFixture() {
  const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
  const manager = new GraphManager(registry);
  const state = new GraphState(manager);
  return { manager, state };
}

describe('clearSelection', () => {
  it('empties selectedNodes', () => {
    const { state } = createFixture();
    state.selectedNodes.add(1);
    state.selectedNodes.add(2);
    state.clearSelection();
    expect(state.selectedNodes.size).toBe(0);
  });
});

describe('projectScreenToWorld', () => {
  it('maps the viewport centre to the camera position', () => {
    const { state } = createFixture();
    // cameraPosition default: [140, 100, 3.5], width=100, height=100
    state.width = 100;
    state.height = 100;
    state.cameraPosition = [140, 100, 3.5];
    const [wx, wy] = state.projectScreenToWorld(50, 50);
    expect(wx).toBeCloseTo(140);
    expect(wy).toBeCloseTo(100);
  });

  it('offsets correctly for a point not at centre', () => {
    const { state } = createFixture();
    state.width = 100;
    state.height = 100;
    state.cameraPosition = [0, 0, 2];
    const [wx, wy] = state.projectScreenToWorld(100, 50);
    // x: 0 + (100 - 50) / 2 = 25
    expect(wx).toBeCloseTo(25);
    expect(wy).toBeCloseTo(0);
  });
});

describe('groupSelectedNodes', () => {
  it('delegates to graph.groupNodes with selected IDs and activeNodeId', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    state.selectedNodes.add(nodeA!.id);
    state.activeNodeId = nodeB!.id;

    const groupNode = state.groupSelectedNodes();
    assert.isDefined(groupNode);

    const graph = manager.serialize();
    expect(graph.groups.length).toBe(1);
    expect(graph.nodes.map(n => n.id)).toContain(groupNode!.id);
  });

  it('works when only activeNodeId is set with no extra selection', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);

    state.activeNodeId = nodeA!.id;
    const groupNode = state.groupSelectedNodes();
    assert.isDefined(groupNode);

    expect(manager.graph.groups.length).toBe(1);
  });
});

describe('enterGroupNode', () => {
  it('does nothing when activeNodeId is -1', () => {
    const { manager, state } = createFixture();
    state.activeNodeId = -1;
    state.enterGroupNode();
    expect(manager.parentStack.length).toBe(0);
  });

  it('does nothing when the active node is not a group instance', () => {
    const { manager, state } = createFixture();
    const node = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(node);
    state.activeNodeId = node!.id;
    state.enterGroupNode();
    expect(manager.parentStack.length).toBe(0);
  });

  it('enters the group, pushes graphStack, and clears UI state', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA!.id]);
    assert.isDefined(groupNode);

    state.selectedNodes.add(nodeA!.id);
    state.activeNodeId = groupNode!.id;
    state.cameraPosition = [10, 20, 5];

    state.enterGroupNode();

    expect(manager.parentStack.length).toBe(1);
    expect(state.activeNodeId).toBe(-1);
    expect(state.selectedNodes.size).toBe(0);
    expect(manager.isInsideGroup).toBe(true);
  });
});

describe('exitGroupNode', () => {
  it('does nothing when not inside a group', () => {
    const { manager, state } = createFixture();
    const before = [...state.cameraPosition];
    state.exitGroupNode();
    expect(manager.parentStack.length).toBe(0);
    expect(state.cameraPosition).toEqual(before);
  });

  it('restores the camera position from before entry', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA!.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode!.id;
    state.cameraPosition = [77, 88, 4];

    state.enterGroupNode();
    // Simulate camera moving inside the group
    state.cameraPosition = [0, 0, 1];

    state.exitGroupNode();

    expect(state.cameraPosition).toEqual([77, 88, 4]);
  });

  it('clears activeNodeId and selection after exit', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA!.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode!.id;
    state.enterGroupNode();
    state.activeNodeId = 99;
    state.selectedNodes.add(99);

    state.exitGroupNode();

    // Group instance node is re-selected on exit; internal selection is cleared
    expect(state.activeNodeId).toBe(groupNode!.id);
    expect(state.selectedNodes.size).toBe(0);
  });

  it('restores outer nodes to manager after exit', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    manager.createEdge(nodeA!, 0, nodeB!, 'value');
    const groupNode = manager.groupNodes([nodeA!.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode!.id;
    state.enterGroupNode();

    // Inside the group: nodeA is an internal node so it IS active; the outer
    // nodes (nodeB, groupNode) are saved and no longer in the active Map.
    expect(manager.nodes.has(nodeA!.id)).toBe(true);
    expect(manager.nodes.has(nodeB!.id)).toBe(false);

    state.exitGroupNode();

    // After exit: outer nodes are restored
    expect(manager.nodes.has(nodeB!.id)).toBe(true);
    expect(manager.nodes.has(groupNode!.id)).toBe(true);
    expect(manager.isInsideGroup).toBe(false);
  });

  it('isInsideGroup is false after exiting the only group level', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA!.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode!.id;
    state.enterGroupNode();
    expect(manager.isInsideGroup).toBe(true);

    state.exitGroupNode();
    expect(manager.isInsideGroup).toBe(false);
  });
});

describe('copyNodes / pasteNodes', () => {
  it('copies the active node into the clipboard', () => {
    const { manager, state } = createFixture();
    const node = manager.createNode({ type: 'test/node/output', position: [10, 20], props: {} });
    assert.isDefined(node);

    state.activeNodeId = node!.id;
    state.mousePosition = [0, 0];
    state.copyNodes();

    assert.isNotNull(state.clipboard);
    expect(state.clipboard!.nodes.map(n => n.id)).toContain(node!.id);
  });

  it('includes edges between copied nodes', () => {
    const { manager, state } = createFixture();
    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    manager.createEdge(nodeA!, 0, nodeB!, 'value');

    state.activeNodeId = nodeA!.id;
    state.selectedNodes.add(nodeB!.id);
    state.mousePosition = [0, 0];
    state.copyNodes();

    assert.isNotNull(state.clipboard);
    expect(state.clipboard!.edges.length).toBe(1);
  });

  it('pastes nodes and adds them to the graph', () => {
    const { manager, state } = createFixture();
    const node = manager.createNode({ type: 'test/node/output', position: [10, 20], props: {} });
    assert.isDefined(node);

    state.activeNodeId = node!.id;
    state.mousePosition = [0, 0];
    state.copyNodes();

    const countBefore = manager.nodes.size;
    state.mousePosition = [50, 50];
    state.pasteNodes();

    expect(manager.nodes.size).toBe(countBefore + 1);
  });

  it('does nothing when clipboard is empty', () => {
    const { manager, state } = createFixture();
    manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const countBefore = manager.nodes.size;
    state.pasteNodes();
    expect(manager.nodes.size).toBe(countBefore);
  });
});
