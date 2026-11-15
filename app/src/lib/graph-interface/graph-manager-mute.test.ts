import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { createMockNodeRegistry, mockFloatInputNode, mockFloatOutputNode } from './test-utils';

function createChain() {
  const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
  const manager = new GraphManager(registry);
  const nodeA = manager.createNode({ type: 'test/node/input', position: [0, 0], props: {} })!;
  const nodeB = manager.createNode({ type: 'test/node/input', position: [10, 0], props: {} })!;
  const output = manager.createNode({ type: 'test/node/output', position: [20, 0], props: {} })!;
  manager.createEdge(nodeA, 0, nodeB, 'value');
  manager.createEdge(nodeB, 0, output, 'input');
  return { manager, nodeA, nodeB, output };
}

const mutedOf = (manager: GraphManager, id: number) =>
  manager.serialize().nodes.find(n => n.id === id)?.muted;

describe('GraphManager.muteNode', () => {
  it('toggles the muted state', () => {
    const { manager, nodeB } = createChain();

    manager.muteNode(nodeB.id);
    expect(manager.getNode(nodeB.id)?.muted).toBe(true);

    manager.muteNode(nodeB.id);
    expect(manager.getNode(nodeB.id)?.muted).toBe(false);
  });

  it('does nothing for an unknown node id', () => {
    const { manager } = createChain();
    const before = JSON.stringify(manager.serialize());

    manager.muteNode(-1);
    manager.muteNode(9999);

    expect(JSON.stringify(manager.serialize())).toBe(before);
  });

  it('does not mute output nodes', () => {
    const { manager, output } = createChain();
    manager.muteNode(output.id);
    expect(manager.getNode(output.id)?.muted).toBeFalsy();
  });

  it('does not mute nodes without inputs', () => {
    const registry = createMockNodeRegistry([
      mockFloatOutputNode,
      { ...mockFloatInputNode, id: 'test/node/noinputs', inputs: {} }
    ]);
    const manager = new GraphManager(registry);
    const node = manager.createNode({ type: 'test/node/noinputs', position: [0, 0], props: {} })!;

    manager.muteNode(node.id);

    expect(manager.getNode(node.id)?.muted).toBeFalsy();
  });

  it('does not mute debug nodes', () => {
    const registry = createMockNodeRegistry([
      mockFloatOutputNode,
      { ...mockFloatInputNode, id: 'test/node/debug' }
    ]);
    const manager = new GraphManager(registry);
    const node = manager.createNode({ type: 'test/node/debug', position: [0, 0], props: {} })!;

    manager.muteNode(node.id);

    expect(manager.getNode(node.id)?.muted).toBeFalsy();
  });

  it('does not mute group boundary nodes', () => {
    const { manager, nodeB } = createChain();
    const group = manager.groupNodes([nodeB.id])!;
    manager.enterGroup(group.id);

    for (const boundary of manager.nodes.values()) {
      if (!boundary.type.startsWith('__internal/group/')) continue;
      manager.muteNode(boundary.id);
      expect(manager.getNode(boundary.id)?.muted).toBeFalsy();
    }
  });

  it('mutes group instances and passes through the first group input', () => {
    const { manager, nodeB } = createChain();
    const group = manager.groupNodes([nodeB.id])!;

    const groupInputs = manager.getGroup(group.props!.groupId as number)!.inputs;
    expect(manager.getMutePassthroughInput(group!)).toBe(Object.keys(groupInputs ?? {})[0]);

    manager.muteNode(group.id);
    expect(manager.getNode(group.id)?.muted).toBe(true);
    expect(mutedOf(manager, group.id)).toBe(true);

    manager.muteNode(group.id);
    expect(manager.getNode(group.id)?.muted).toBeFalsy();
  });

  it('does not mute a group instance without inputs', () => {
    const { manager, nodeA } = createChain();
    const group = manager.groupNodes([nodeA.id])!;
    expect(manager.getGroup(group.props!.groupId as number)!.inputs).toEqual({});

    manager.muteNode(group.id);

    expect(manager.getNode(group.id)?.muted).toBeFalsy();
  });

  it('serializes muted only for muted nodes', () => {
    const { manager, nodeA, nodeB } = createChain();
    manager.muteNode(nodeB.id);

    expect(mutedOf(manager, nodeB.id)).toBe(true);
    expect(mutedOf(manager, nodeA.id)).toBeFalsy();
  });

  it('keeps the muted state through a serialize → load round trip', async () => {
    const { manager, nodeA, nodeB } = createChain();
    manager.muteNode(nodeB.id);
    const graph = JSON.parse(JSON.stringify(manager.serialize())) as Graph;

    const loaded = new GraphManager(manager.registry);
    await loaded.load(graph);

    expect(loaded.getNode(nodeB.id)?.muted).toBe(true);
    expect(loaded.getNode(nodeA.id)?.muted).toBeFalsy();
  });

  it('can undo and redo muting', () => {
    const { manager, nodeB } = createChain();

    manager.muteNode(nodeB.id);
    expect(manager.getNode(nodeB.id)?.muted).toBe(true);

    manager.undo();
    expect(manager.getNode(nodeB.id)?.muted).toBeFalsy();

    manager.redo();
    expect(manager.getNode(nodeB.id)?.muted).toBe(true);
  });

  it('keeps edges and props when muting', () => {
    const { manager, nodeB } = createChain();
    const before = manager.serialize();

    manager.muteNode(nodeB.id);
    const after = manager.serialize();

    expect(after.edges).toEqual(before.edges);
    expect(after.nodes.find(n => n.id === nodeB.id)?.props).toEqual(
      before.nodes.find(n => n.id === nodeB.id)?.props
    );
  });

  it('keeps a muted node muted when it is grouped', () => {
    const { manager, nodeA, nodeB } = createChain();
    manager.muteNode(nodeB.id);

    const group = manager.groupNodes([nodeA.id, nodeB.id])!;
    const serialized = manager.serialize();
    const inner = serialized.groups.find(g => g.id === group.props?.groupId)?.nodes.find(n =>
      n.muted
    );

    expect(inner).toBeDefined();
  });

  it('keeps a muted node muted when its group is ungrouped', () => {
    const { manager, nodeA, nodeB, output } = createChain();
    manager.muteNode(nodeB.id);
    const group = manager.groupNodes([nodeA.id, nodeB.id])!;

    manager.ungroupNode(group.id);

    const graph = manager.serialize();
    const muted = graph.nodes.filter(n => n.muted);
    expect(muted).toHaveLength(1);
    expect(graph.edges).toContainEqual([muted[0].id, 0, output.id, 'input']);
    expect(manager.getNode(muted[0].id)?.muted).toBe(true);
  });

  it('keeps a pasted muted node muted', () => {
    const { manager, nodeB } = createChain();
    manager.muteNode(nodeB.id);
    const copied = manager.serialize().nodes.find(n => n.id === nodeB.id)!;

    const [pasted] = manager.createGraph([copied], []);

    expect(pasted.muted).toBe(true);
    expect(mutedOf(manager, pasted.id)).toBe(true);
  });

  it('keeps the muted state of nodes inside a group after leaving it', () => {
    const { manager, nodeA, nodeB } = createChain();
    const group = manager.groupNodes([nodeA.id, nodeB.id])!;
    manager.enterGroup(group.id);
    const inner = [...manager.nodes.values()].find(n => n.type === 'test/node/input')!;

    manager.muteNode(inner.id);
    manager.exitGroup();

    const stored = manager.serialize().groups.flatMap(g => g.nodes).find(n => n.id === inner.id);
    expect(stored?.muted).toBe(true);
  });
});

describe('GraphManager.muteNode with unwirable first inputs', () => {
  it('does not mute a node whose first input is a seed or setting', () => {
    for (const input of [{ type: 'seed' }, { type: 'float', setting: 'foo' }]) {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        { ...mockFloatInputNode, id: 'test/node/seeded', inputs: { first: input } } as never
      ]);
      const manager = new GraphManager(registry);
      const node = manager.createNode({ type: 'test/node/seeded', position: [0, 0], props: {} })!;

      manager.muteNode(node.id);

      expect(manager.getNode(node.id)?.muted).toBeFalsy();
    }
  });
});
