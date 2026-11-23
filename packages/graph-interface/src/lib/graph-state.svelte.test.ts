import { assert, describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { GraphState } from './graph-state.svelte';
import {
  createGraphOptions,
  createMockNodeRegistry,
  mockFloatInputNode,
  mockFloatOutputNode
} from './test-utils';

function createFixture() {
  const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
  const manager = new GraphManager(registry);
  const options = createGraphOptions();
  const state = new GraphState(manager, () => options);
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

describe('groupSelectedNodes', () => {
  it('delegates to graph.groupNodes with selected IDs and activeNodeId', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    state.selectedNodes.add(nodeA.id);
    state.activeNodeId = nodeB.id;

    const groupNode = state.groupSelectedNodes();
    assert.isDefined(groupNode);

    const graph = manager.serialize();
    expect(graph.groups.length).toBe(1);
    expect(graph.nodes.map(n => n.id)).toContain(groupNode.id);
  });

  it('works when only activeNodeId is set with no extra selection', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);

    state.activeNodeId = nodeA.id;
    const groupNode = state.groupSelectedNodes();
    assert.isDefined(groupNode);

    expect(manager.groups.length).toBe(1);
  });
});

describe('enterGroupNode', () => {
  it('does nothing when activeNodeId is -1', () => {
    const { manager, state } = createFixture();
    state.activeNodeId = -1;
    state.enterGroupNode();
    expect(manager.path.length).toBe(0);
  });

  it('does nothing when the active node is not a group instance', () => {
    const { manager, state } = createFixture();
    const node = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(node);
    state.activeNodeId = node.id;
    state.enterGroupNode();
    expect(manager.path.length).toBe(0);
  });

  it('enters the group and clears UI state', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA.id]);
    assert.isDefined(groupNode);

    state.selectedNodes.add(nodeA.id);
    state.activeNodeId = groupNode.id;
    state.camera.position = [10, 20, 5];

    state.enterGroupNode();

    expect(manager.path.length).toBe(1);
    expect(state.activeNodeId).toBe(-1);
    expect(state.selectedNodes.size).toBe(0);
    expect(manager.isInsideGroup).toBe(true);
  });
});

describe('exitGroupNode', () => {
  it('does nothing when not inside a group', () => {
    const { manager, state } = createFixture();
    const before = [...state.camera.position];
    state.exitGroupNode();
    expect(manager.path.length).toBe(0);
    expect(state.camera.position).toEqual(before);
  });

  it('clears activeNodeId and selection after exit', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode.id;
    state.enterGroupNode();
    state.activeNodeId = 99;
    state.selectedNodes.add(99);

    state.exitGroupNode();

    // Group instance node is re-selected on exit; internal selection is cleared
    expect(state.activeNodeId).toBe(groupNode.id);
    expect(state.selectedNodes.size).toBe(0);
  });

  it('restores outer nodes to manager after exit', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    manager.createEdge(nodeA!, 0, nodeB!, 'value');
    const groupNode = manager.groupNodes([nodeA.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode.id;
    state.enterGroupNode();

    // Inside the group: nodeA is an internal node so it IS active; the outer
    // nodes (nodeB, groupNode) are saved and no longer in the active Map.
    expect(manager.nodes.has(nodeA.id)).toBe(true);
    expect(manager.nodes.has(nodeB.id)).toBe(false);

    state.exitGroupNode();

    // After exit: outer nodes are restored
    expect(manager.nodes.has(nodeB.id)).toBe(true);
    expect(manager.nodes.has(groupNode.id)).toBe(true);
    expect(manager.isInsideGroup).toBe(false);
  });

  it('isInsideGroup is false after exiting the only group level', () => {
    const { manager, state } = createFixture();

    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(nodeA);
    const groupNode = manager.groupNodes([nodeA.id]);
    assert.isDefined(groupNode);

    state.activeNodeId = groupNode.id;
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

    state.activeNodeId = node.id;
    state.mousePosition = [0, 0];
    state.copyNodes();

    assert.isNotNull(state.clipboard);
    expect(state.clipboard!.nodes.map(n => n.id)).toContain(node.id);
  });

  it('includes edges between copied nodes', () => {
    const { manager, state } = createFixture();
    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/input', position: [100, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);

    manager.createEdge(nodeA!, 0, nodeB!, 'value');

    state.activeNodeId = nodeA.id;
    state.selectedNodes.add(nodeB.id);
    state.mousePosition = [0, 0];
    state.copyNodes();

    assert.isNotNull(state.clipboard);
    expect(state.clipboard!.edges.length).toBe(1);
  });

  it('pastes nodes and adds them to the graph', () => {
    const { manager, state } = createFixture();
    const node = manager.createNode({ type: 'test/node/output', position: [10, 20], props: {} });
    assert.isDefined(node);

    state.activeNodeId = node.id;
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

describe('connecting without a drag', () => {
  function setup() {
    const { manager, state } = createFixture();
    const from = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const to = manager.createNode({ type: 'test/node/input', position: [50, 0], props: {} });
    assert.isDefined(from);
    assert.isDefined(to);
    const output = { node: from, index: 0, position: [0, 0] as [number, number] };
    const input = { node: to, index: 'value', position: [0, 0] as [number, number] };
    const at = (x: number) => ({ clientX: x, clientY: 10 });
    const edges = () => manager.serialize().edges;
    return { manager, state, from, to, output, input, at, edges };
  }

  it('a click on an output picks it and keeps the inputs it fits highlighted', () => {
    const { state, to, output, at } = setup();
    state.pressSocket(output, at(10));
    const clicked = state.releasedSocket(at(11));
    assert.isNotNull(clicked);
    state.pickSocket(clicked);

    expect(state.isPicked(output.node, 0)).toBe(true);
    expect(state.possibleSocketIds.has(`${to.id}-value`)).toBe(true);
  });

  it('a drag does not pick, also when it ends where it started', () => {
    const { state, output, at } = setup();
    state.pressSocket(output, at(10));
    expect(state.releasedSocket(at(80))).toBeNull();

    state.pressSocket(output, at(10));
    state.movePointer(at(80));
    expect(state.releasedSocket(at(10))).toBeNull();
  });

  it('a click on the picked socket unpicks it, a drag from it still drags', () => {
    const { state, output, at } = setup();
    state.pickSocket(output);
    state.pressSocket(output, at(10));
    expect(state.pickedSocket).toBeNull();
    expect(state.activeSocket?.node.id).toBe(output.node.id);
    expect(state.releasedSocket(at(10))).toBeNull();
  });

  it('a click on a highlighted input then makes the edge and leaves the picked state', () => {
    const { state, from, to, output, input, at, edges } = setup();
    state.pickSocket(output);
    state.pressSocket(input, at(10));

    expect(edges()).toEqual([[from.id, 0, to.id, 'value']]);
    expect(state.pickedSocket).toBeNull();
    expect(state.possibleSockets).toEqual([]);
    expect(state.activeSocket).toBeNull();
  });

  it('pressing a connected input drags its edge away as before and picks nothing', () => {
    const { manager, state, from, to, input, at, edges } = setup();
    manager.createEdge(from, 0, to, 'value');
    state.pressSocket(input, at(10));

    expect(edges()).toEqual([]);
    expect(state.releasedSocket(at(10))).toBeNull();
  });

  it('Enter on an output and then on an input makes the edge', () => {
    const { state, from, to, output, input, edges } = setup();
    state.activateSocket(output);
    state.activateSocket(input);
    expect(edges()).toEqual([[from.id, 0, to.id, 'value']]);
  });

  it('Enter on an input and then on an output makes the edge', () => {
    const { state, from, to, output, input, edges } = setup();
    state.activateSocket(input);
    expect(state.possibleSocketIds.has(`${from.id}-0`)).toBe(true);
    state.activateSocket(output);
    expect(edges()).toEqual([[from.id, 0, to.id, 'value']]);
  });

  it('Enter on the picked socket again leaves the picked state', () => {
    const { state, output, edges } = setup();
    state.activateSocket(output);
    state.activateSocket(output);
    expect(state.pickedSocket).toBeNull();
    expect(state.possibleSockets).toEqual([]);
    expect(edges()).toEqual([]);
  });

  it('a socket that does not fit is picked instead', () => {
    const { state, from, output } = setup();
    state.activateSocket(output);
    state.activateSocket({ node: from, index: 'input', position: [0, 0] });
    expect(state.isPicked(from, 'input')).toBe(true);
  });
});
