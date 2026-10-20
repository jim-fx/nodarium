import { debugNode } from '$lib/node-registry/debugNode';
import { expandGroups } from '$lib/runtime/runtime-executor';
import type { NodeDefinition } from '@nodarium/types';
import { assert, describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { getNodeHeight } from './helpers/nodeHelpers';
import {
  createMockNodeRegistry,
  mockFloatInputNode,
  mockFloatOutputNode,
  mockGeometryOutputNode,
  mockPathInputNode,
  mockVec3OutputNode
} from './test-utils';

describe('groupNodes', () => {
  it('should not do anything if no nodes are selected', () => {
    const registry = createMockNodeRegistry([
      mockFloatOutputNode,
      mockFloatInputNode,
      mockGeometryOutputNode,
      mockPathInputNode
    ]);

    const manager = new GraphManager(registry);

    const floatInputNode = manager.createNode({
      type: 'test/node/input',
      position: [100, 100],
      props: {}
    });

    assert.isDefined(floatInputNode);

    const floatOutputNode = manager.createNode({
      type: 'test/node/output',
      position: [0, 0],
      props: {}
    });
    assert.isDefined(floatOutputNode);

    const edge = manager.createEdge(floatInputNode, 0, floatOutputNode, 'input');
    assert.isDefined(edge);
    manager.save();

    manager.groupNodes([]);

    const graph = manager.serialize();
    expect(graph.nodes.length).toBe(2);
    expect(graph.edges.length).toBe(1);
    expect(graph.groups.length).toBe(0);
  });

  it('should group selected nodes and create a group node', () => {
    const registry = createMockNodeRegistry([
      mockFloatOutputNode,
      mockFloatInputNode,
      mockGeometryOutputNode,
      mockPathInputNode
    ]);

    const manager = new GraphManager(registry);

    const floatInputNode = manager.createNode({
      type: 'test/node/input',
      position: [100, 100],
      props: {}
    });

    assert.isDefined(floatInputNode);

    const floatOutputNode = manager.createNode({
      type: 'test/node/output',
      position: [0, 0],
      props: {}
    });
    assert.isDefined(floatOutputNode);

    const edge = manager.createEdge(floatInputNode, 0, floatOutputNode, 'input');
    assert.isDefined(edge);
    manager.save();

    const groupNode = manager.groupNodes([floatInputNode.id]);
    assert.isDefined(groupNode);

    const graph = manager.serialize();

    expect(graph.nodes.map(n => n.id), 'graph to contain group node').to.contain(groupNode.id);
    expect(graph.groups[0].nodes.map(n => n.id), 'group graph to contain float node').to.contain(
      floatInputNode.id
    );
    expect(graph.nodes.map(n => n.id)).not.to.contain(floatInputNode.id);

    expect(graph.nodes.length).toBe(2);
    expect(graph.edges.length).toBe(1);
    expect(graph.groups.length).toBe(1);
  });

  it('should rewire external edges when grouping a middle node in a chain', () => {
    const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
    const manager = new GraphManager(registry);

    // A → B → C  (float chain: output → middle → input)
    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/output', position: [100, 0], props: {} });
    const nodeC = manager.createNode({ type: 'test/node/input', position: [200, 0], props: {} });

    assert.isDefined(nodeA);
    assert.isDefined(nodeB);
    assert.isDefined(nodeC);

    manager.createEdge(nodeA, 0, nodeB, 'input');
    manager.createEdge(nodeB, 0, nodeC, 'value');

    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    const graph = manager.serialize();

    // Top-level: A, C, groupNode — B is gone
    expect(graph.nodes.length, 'top-level node count').toBe(3);
    const topLevelIds = graph.nodes.map(n => n.id);
    expect(topLevelIds).toContain(nodeA.id);
    expect(topLevelIds).toContain(nodeC.id);
    expect(topLevelIds).toContain(groupNode.id);
    expect(topLevelIds).not.toContain(nodeB.id);

    // Both original edges survive, now routing through the group node
    expect(graph.edges.length, 'edge count unchanged').toBe(2);
    const edgeSources = graph.edges.map(e => e[0]);
    const edgeTargets = graph.edges.map(e => e[2]);
    expect(edgeTargets).toContain(groupNode.id); // A → groupNode
    expect(edgeSources).toContain(groupNode.id); // groupNode → C

    // One group definition was created
    expect(graph.groups.length).toBe(1);
    const group = graph.groups[0];

    // Group contains B plus the two boundary nodes
    const groupNodeIds = group.nodes.map(n => n.id);
    expect(groupNodeIds).toContain(nodeB.id);
    const inputBoundary = group.nodes.find(n => n.type === '__internal/group/input');
    const outputBoundary = group.nodes.find(n => n.type === '__internal/group/output');
    expect(inputBoundary, 'group input boundary node').toBeDefined();
    expect(outputBoundary, 'group output boundary node').toBeDefined();

    // Group declares one input slot and one output slot
    expect(Object.keys(group.inputs ?? {}).length, 'group input count').toBe(1);
    expect(group.outputs?.length, 'group output count').toBe(1);

    // Internal edges wire: inputBoundary → B → outputBoundary
    expect(group.edges.length, 'internal edge count').toBe(2);
    const internalSources = group.edges.map(e => e[0]);
    const internalTargets = group.edges.map(e => e[2]);
    expect(internalTargets).toContain(nodeB.id);
    expect(internalSources).toContain(nodeB.id);
  });
});

describe('getPossibleSockets', () => {
  describe('when dragging an output socket', () => {
    it('should return compatible input sockets based on type', () => {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        mockFloatInputNode,
        mockGeometryOutputNode,
        mockPathInputNode
      ]);

      const manager = new GraphManager(registry);

      const floatInputNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      const floatOutputNode = manager.createNode({
        type: 'test/node/output',
        position: [0, 0],
        props: {}
      });

      expect(floatInputNode).toBeDefined();
      expect(floatOutputNode).toBeDefined();

      const possibleSockets = manager.getPossibleSockets({
        node: floatOutputNode!,
        index: 0,
        position: [0, 0]
      });

      expect(possibleSockets.length).toBe(1);
      const socketNodeIds = possibleSockets.map(([node]) => node.id);
      expect(socketNodeIds).toContain(floatInputNode!.id);
    });

    it('should exclude self node from possible sockets', () => {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        mockFloatInputNode
      ]);

      const manager = new GraphManager(registry);

      const floatInputNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      expect(floatInputNode).toBeDefined();

      const possibleSockets = manager.getPossibleSockets({
        node: floatInputNode!,
        index: 'value',
        position: [0, 0]
      });

      const socketNodeIds = possibleSockets.map(([node]) => node.id);
      expect(socketNodeIds).not.toContain(floatInputNode!.id);
    });

    it('should exclude parent nodes from possible sockets when dragging output', () => {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        mockFloatInputNode
      ]);

      const manager = new GraphManager(registry);

      const parentNode = manager.createNode({
        type: 'test/node/output',
        position: [0, 0],
        props: {}
      });

      const childNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      expect(parentNode).toBeDefined();
      expect(childNode).toBeDefined();

      if (parentNode && childNode) {
        manager.createEdge(parentNode, 0, childNode, 'value');
      }

      const possibleSockets = manager.getPossibleSockets({
        node: parentNode!,
        index: 0,
        position: [0, 0]
      });

      const socketNodeIds = possibleSockets.map(([node]) => node.id);
      expect(socketNodeIds).not.toContain(childNode!.id);
    });

    it('should return sockets compatible with accepts property', () => {
      const registry = createMockNodeRegistry([
        mockGeometryOutputNode,
        mockPathInputNode
      ]);

      const manager = new GraphManager(registry);

      const geometryOutputNode = manager.createNode({
        type: 'test/node/geometry',
        position: [0, 0],
        props: {}
      });

      const pathInputNode = manager.createNode({
        type: 'test/node/path',
        position: [100, 100],
        props: {}
      });

      expect(geometryOutputNode).toBeDefined();
      expect(pathInputNode).toBeDefined();

      const possibleSockets = manager.getPossibleSockets({
        node: geometryOutputNode!,
        index: 0,
        position: [0, 0]
      });

      const socketNodeIds = possibleSockets.map(([node]) => node.id);
      expect(socketNodeIds).toContain(pathInputNode!.id);
    });

    it('should return empty array when no compatible sockets exist', () => {
      const registry = createMockNodeRegistry([
        mockVec3OutputNode,
        mockFloatInputNode
      ]);

      const manager = new GraphManager(registry);

      const vec3OutputNode = manager.createNode({
        type: 'test/node/vec3',
        position: [0, 0],
        props: {}
      });

      const floatInputNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      expect(vec3OutputNode).toBeDefined();
      expect(floatInputNode).toBeDefined();

      const possibleSockets = manager.getPossibleSockets({
        node: vec3OutputNode!,
        index: 0,
        position: [0, 0]
      });

      const socketNodeIds = possibleSockets.map(([node]) => node.id);
      expect(socketNodeIds).not.toContain(floatInputNode!.id);
      expect(possibleSockets.length).toBe(0);
    });

    it('should return socket info with correct socket key for inputs', () => {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        mockFloatInputNode
      ]);

      const manager = new GraphManager(registry);

      const floatOutputNode = manager.createNode({
        type: 'test/node/output',
        position: [0, 0],
        props: {}
      });

      const floatInputNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      expect(floatOutputNode).toBeDefined();
      expect(floatInputNode).toBeDefined();

      const possibleSockets = manager.getPossibleSockets({
        node: floatOutputNode!,
        index: 0,
        position: [0, 0]
      });

      const matchingSocket = possibleSockets.find(([node]) => node.id === floatInputNode!.id);
      expect(matchingSocket).toBeDefined();
      expect(matchingSocket![1]).toBe('value');
    });

    it('should return multiple compatible sockets', () => {
      const registry = createMockNodeRegistry([
        mockFloatOutputNode,
        mockFloatInputNode,
        mockGeometryOutputNode,
        mockPathInputNode
      ]);

      const manager = new GraphManager(registry);

      const floatOutputNode = manager.createNode({
        type: 'test/node/output',
        position: [0, 0],
        props: {}
      });

      const geometryOutputNode = manager.createNode({
        type: 'test/node/geometry',
        position: [200, 0],
        props: {}
      });

      const floatInputNode = manager.createNode({
        type: 'test/node/input',
        position: [100, 100],
        props: {}
      });

      const pathInputNode = manager.createNode({
        type: 'test/node/path',
        position: [300, 100],
        props: {}
      });

      expect(floatOutputNode).toBeDefined();
      expect(geometryOutputNode).toBeDefined();
      expect(floatInputNode).toBeDefined();
      expect(pathInputNode).toBeDefined();

      const possibleSocketsForFloat = manager.getPossibleSockets({
        node: floatOutputNode!,
        index: 0,
        position: [0, 0]
      });

      expect(possibleSocketsForFloat.length).toBe(1);
      expect(possibleSocketsForFloat.map(([n]) => n.id)).toContain(floatInputNode!.id);
    });
  });
});

describe('groups', () => {
  function createChain() {
    const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
    const manager = new GraphManager(registry);

    // A → B → C
    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/output', position: [100, 0], props: {} });
    const nodeC = manager.createNode({ type: 'test/node/input', position: [200, 0], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);
    assert.isDefined(nodeC);
    manager.createEdge(nodeA, 0, nodeB, 'input');
    manager.createEdge(nodeB, 0, nodeC, 'value');

    return { manager, nodeA, nodeB, nodeC };
  }

  it('should restore the group definitions on undo and redo', () => {
    const { manager, nodeB } = createChain();

    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    expect(manager.groups.length).toBe(1);

    manager.undo();
    expect(manager.groups.length).toBe(0);
    expect(manager.nodes.has(nodeB.id)).toBe(true);
    expect(manager.nodes.has(groupNode.id)).toBe(false);

    manager.redo();
    expect(manager.groups.length).toBe(1);
    const restoredGroupNode = manager.getNode(groupNode.id);
    assert.isDefined(restoredGroupNode);
    expect(manager.getNodeType(restoredGroupNode)?.outputs).toEqual(['float']);
  });

  it('should undo inside a group without loading the root nodes into it', () => {
    const { manager, nodeA, nodeB, nodeC } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    expect(manager.enterGroup(groupNode.id)).toBe(true);
    const groupLevelTypes = [...manager.nodes.values()].map(n => n.type).sort();

    // The first edit after entering must be undoable too
    manager.createNode({ type: 'test/node/output', position: [50, 50], props: {} });
    expect(manager.nodes.size).toBe(groupLevelTypes.length + 1);

    manager.undo();
    expect([...manager.nodes.values()].map(n => n.type).sort()).toEqual(groupLevelTypes);

    manager.exitGroup();
    const graph = manager.serialize();
    expect(graph.nodes.map(n => n.id).sort()).toEqual([nodeA.id, nodeC.id, groupNode.id].sort());
    expect(graph.groups[0].nodes.map(n => n.type).sort()).toEqual(groupLevelTypes);
  });

  it('should not delete unused groups when grouping other nodes', () => {
    const { manager, nodeB, nodeC } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    manager.removeNode(groupNode);

    manager.groupNodes([nodeC.id]);
    expect(manager.groups.length).toBe(2);
  });

  it('should keep saving after grouping an empty selection', () => {
    const { manager } = createChain();
    manager.groupNodes([-1]);
    expect(manager.currentUndoGroup).toBeNull();
  });

  it('should create one group output per internal source socket', () => {
    const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
    const manager = new GraphManager(registry);

    // A → B ┬→ C       B and E get grouped, B fans out to C and D
    //       └→ D
    //     E ──→ F
    const nodeA = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/output', position: [100, 0], props: {} });
    const nodeE = manager.createNode({ type: 'test/node/input', position: [100, 100], props: {} });
    const nodeC = manager.createNode({ type: 'test/node/input', position: [200, 0], props: {} });
    const nodeD = manager.createNode({ type: 'test/node/output', position: [200, 50], props: {} });
    const nodeF = manager.createNode({ type: 'test/node/output', position: [200, 100], props: {} });
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);
    assert.isDefined(nodeE);
    assert.isDefined(nodeC);
    assert.isDefined(nodeD);
    assert.isDefined(nodeF);
    manager.createEdge(nodeA, 0, nodeB, 'input');
    manager.createEdge(nodeB, 0, nodeC, 'value');
    manager.createEdge(nodeB, 0, nodeD, 'input');
    manager.createEdge(nodeE, 0, nodeF, 'input');

    const groupNode = manager.groupNodes([nodeB.id, nodeE.id]);
    assert.isDefined(groupNode);

    const graph = manager.serialize();
    const group = graph.groups[0];
    expect(group.outputs?.map(o => o.type)).toEqual(['float', 'float']);

    const outputBoundary = group.nodes.find(n => n.type === '__internal/group/output')!;
    expect(group.edges.filter(e => e[2] === outputBoundary.id)).toEqual([
      [nodeB.id, 0, outputBoundary.id, 'out_0'],
      [nodeE.id, 0, outputBoundary.id, 'out_1']
    ]);

    expect(graph.edges).toHaveLength(4);
    expect(graph.edges).toContainEqual([groupNode.id, 0, nodeC.id, 'value']);
    expect(graph.edges).toContainEqual([groupNode.id, 0, nodeD.id, 'input']);
    expect(graph.edges).toContainEqual([groupNode.id, 1, nodeF.id, 'input']);

    // The runtime must not cross-wire the two outputs
    const expanded = expandGroups(graph);
    const sourceOf = (id: number) =>
      expanded.nodes.find(n => n.id === expanded.edges.find(e => e[2] === id)![0])!;
    expect(expanded.edges).toHaveLength(4);
    expect(sourceOf(nodeC.id).type).toBe('test/node/output');
    expect(sourceOf(nodeD.id).type).toBe('test/node/output');
    expect(sourceOf(nodeF.id).type).toBe('test/node/input');

    // And ungrouping restores the same wiring
    manager.ungroupNode(groupNode.id);
    const ungrouped = manager.serialize();
    const ungroupedSourceOf = (id: number) =>
      ungrouped.nodes.find(n => n.id === ungrouped.edges.find(e => e[2] === id)![0])!;
    expect(ungrouped.edges).toHaveLength(4);
    expect(ungroupedSourceOf(nodeF.id).type).toBe('test/node/input');
    expect(ungroupedSourceOf(nodeC.id).type).toBe('test/node/output');
  });

  it('should give new group nodes their sockets for connecting', () => {
    const { manager, nodeB } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    // P → Q, the group node can be dropped on it
    const nodeP = manager.createNode({ type: 'test/node/output', position: [0, 100], props: {} });
    const nodeQ = manager.createNode({ type: 'test/node/input', position: [200, 100], props: {} });
    assert.isDefined(nodeP);
    assert.isDefined(nodeQ);
    const edgePQ = manager.createEdge(nodeP, 0, nodeQ, 'value');
    assert.isDefined(edgePQ);
    expect(manager.getPossibleDropOnEdges(groupNode.id)).toEqual([edgePQ]);

    const possibleNodes = manager.getPossibleNodes({ node: groupNode, index: 0, position: [0, 0] });
    expect(possibleNodes.map(n => n.id).sort()).toEqual(['test/node/input', 'test/node/output']);

    const nodeR = manager.createNode({ type: 'test/node/input', position: [200, 200], props: {} });
    assert.isDefined(nodeR);
    expect(manager.smartConnect(groupNode, nodeR)).toBeDefined();
  });

  it('should update loaded group nodes when the group definition changes', () => {
    const { manager, nodeB } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    const groupId = groupNode.props?.groupId as number;

    // Reload the level, so the group node is created from the serialized graph
    manager.enterGroup(groupNode.id);
    manager.exitGroup();
    const loadedGroupNode = manager.getNode(groupNode.id);
    assert.isDefined(loadedGroupNode);
    expect(Object.keys(manager.getNodeType(loadedGroupNode)?.inputs ?? {})).toEqual([
      'groupId',
      'input_0'
    ]);

    manager.removeGroupInput(groupId, 'input_0');
    expect(Object.keys(manager.getNodeType(loadedGroupNode)?.inputs ?? {})).toEqual(['groupId']);
  });

  it('should update the parents and children when grouping', () => {
    const { manager, nodeA, nodeB, nodeC } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    expect(manager.getNode(nodeA.id)?.state.children).toEqual([groupNode]);
    expect(groupNode.state.parents).toEqual([manager.getNode(nodeA.id)]);
    expect(groupNode.state.children).toEqual([manager.getNode(nodeC.id)]);
    expect(manager.getNode(nodeC.id)?.state.parents).toEqual([groupNode]);
  });

  it('should start the group input at the value of the internal node', () => {
    const { manager, nodeB } = createChain();
    nodeB.props = { input: 0.25 };
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    const group = manager.getGroup(groupNode.props?.groupId as number);
    expect(group?.inputs?.input_0?.value).toBe(0.25);
  });

  it('should not paste a second group input or output node', () => {
    const { manager, nodeB } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    manager.enterGroup(groupNode.id);

    const level = manager.serialize().groups[0];
    manager.createGraph(level.nodes, level.edges);

    const types = [...manager.nodes.values()].map(n => n.type);
    expect(types.filter(t => t === '__internal/group/input')).toHaveLength(1);
    expect(types.filter(t => t === '__internal/group/output')).toHaveLength(1);
    expect(types.filter(t => t === 'test/node/output')).toHaveLength(2);
  });

  it('should not reuse the IDs of existing nodes when pasting', () => {
    const { manager, nodeA, nodeB, nodeC } = createChain();
    manager.removeNode(nodeB);

    const pasted = manager.createGraph([
      { id: 0, type: 'test/node/output', position: [0, 0] },
      { id: 1, type: 'test/node/output', position: [0, 0] }
    ], [[0, 0, 1, 'input']]);

    expect(manager.nodes.size).toBe(4);
    expect(pasted.map(n => n.id)).not.toContain(nodeA.id);
    expect(pasted.map(n => n.id)).not.toContain(nodeC.id);
    expect(manager.getNode(nodeC.id)?.type).toBe('test/node/input');
    expect(pasted[1].state.parents).toEqual([pasted[0]]);
  });

  it('should not reuse the IDs of the parent levels inside a group', () => {
    const { manager, nodeB } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    const rootIds = [...manager.nodes.keys()];

    manager.enterGroup(groupNode.id);
    const node = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    assert.isDefined(node);
    expect(rootIds).not.toContain(node.id);
  });

  it('should undo an edit made inside a group after leaving it', () => {
    const { manager, nodeB } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);
    const groupNodeCount = manager.serialize().groups[0].nodes.length;

    manager.enterGroup(groupNode.id);
    manager.createNode({ type: 'test/node/output', position: [50, 50], props: {} });
    manager.exitGroup();
    expect(manager.serialize().groups[0].nodes).toHaveLength(groupNodeCount + 1);

    manager.undo();
    expect(manager.isInsideGroup).toBe(false);
    expect(manager.serialize().groups[0].nodes).toHaveLength(groupNodeCount);
  });

  it('should leave a group when undoing to before it existed', () => {
    const { manager, nodeA, nodeB, nodeC } = createChain();
    const groupNode = manager.groupNodes([nodeB.id]);
    assert.isDefined(groupNode);

    manager.enterGroup(groupNode.id);
    manager.undo();

    expect(manager.isInsideGroup).toBe(false);
    expect(manager.parentStack).toHaveLength(0);
    expect([...manager.nodes.keys()].sort()).toEqual([nodeA.id, nodeB.id, nodeC.id].sort());
    expect(manager.serialize().groups).toHaveLength(0);
  });

  it('should keep the parent levels when undoing inside a nested group', () => {
    const { manager, nodeA, nodeB, nodeC } = createChain();
    const outer = manager.groupNodes([nodeB.id]);
    assert.isDefined(outer);
    manager.enterGroup(outer.id);
    const inner = manager.groupNodes([nodeB.id]);
    assert.isDefined(inner);
    manager.enterGroup(inner.id);
    const innerTypes = [...manager.nodes.values()].map(n => n.type).sort();

    manager.createNode({ type: 'test/node/output', position: [50, 50], props: {} });
    manager.undo();

    expect(manager.currentGroupId).toBe(inner.props?.groupId);
    expect(manager.parentStack).toHaveLength(2);
    expect([...manager.nodes.values()].map(n => n.type).sort()).toEqual(innerTypes);

    manager.exitGroup();
    manager.exitGroup();
    const graph = manager.serialize();
    expect(graph.nodes.map(n => n.id).sort()).toEqual([nodeA.id, nodeC.id, outer.id].sort());
  });

  it('should not mistake the root graph for a group with the same id', () => {
    const { manager, nodeB } = createChain();
    const outer = manager.groupNodes([nodeB.id]);
    assert.isDefined(outer);
    const outerGroupId = outer.props?.groupId as number;
    manager.id = outerGroupId;

    manager.enterGroup(outer.id);
    const inner = manager.groupNodes([nodeB.id]);
    assert.isDefined(inner);
    manager.enterGroup(inner.id);

    const graph = manager.serialize();
    expect(graph.nodes.map(n => n.id)).toContain(outer.id);
    const outerGroup = graph.groups.find(g => g.id === outerGroupId);
    expect(outerGroup?.nodes.map(n => n.id)).toContain(inner.id);
  });
});

describe('removeGroupInput', () => {
  // A1 → B, A2 → E, then B and E get grouped into a group with input_0 and input_1
  function createGroupWithTwoInputs() {
    const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode]);
    const manager = new GraphManager(registry);
    const nodeA1 = manager.createNode({ type: 'test/node/output', position: [0, 0], props: {} });
    const nodeA2 = manager.createNode({ type: 'test/node/output', position: [0, 100], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/output', position: [100, 0], props: {} });
    const nodeE = manager.createNode({ type: 'test/node/input', position: [100, 100], props: {} });
    assert.isDefined(nodeA1);
    assert.isDefined(nodeA2);
    assert.isDefined(nodeB);
    assert.isDefined(nodeE);
    manager.createEdge(nodeA1, 0, nodeB, 'input');
    manager.createEdge(nodeA2, 0, nodeE, 'value');

    const groupNode = manager.groupNodes([nodeB.id, nodeE.id]);
    assert.isDefined(groupNode);
    const groupId = groupNode.props?.groupId as number;
    expect(Object.keys(manager.getGroup(groupId)?.inputs ?? {})).toEqual(['input_0', 'input_1']);

    return { manager, groupNode, groupId, nodeA2, nodeE };
  }

  function expectInputRemoved(
    { manager, groupNode, nodeA2, nodeE }: ReturnType<typeof createGroupWithTwoInputs>
  ) {
    const graph = manager.serialize();
    const group = graph.groups[0];
    expect(Object.keys(group.inputs ?? {})).toEqual(['input_1']);

    const inputBoundary = group.nodes.find(n => n.type === '__internal/group/input')!;
    expect(group.edges.filter(e => e[0] === inputBoundary.id)).toEqual([
      [inputBoundary.id, 0, nodeE.id, 'value']
    ]);
    expect(graph.edges).toEqual([[nodeA2.id, 0, groupNode.id, 'input_1']]);

    const expanded = expandGroups(graph);
    expect(expanded.edges).toHaveLength(1);
    expect(expanded.edges[0][0]).toBe(nodeA2.id);
    expect(expanded.nodes.find(n => n.id === expanded.edges[0][2])?.type).toBe('test/node/input');
  }

  it('should shift the later group input sockets', () => {
    const setup = createGroupWithTwoInputs();
    setup.manager.removeGroupInput(setup.groupId, 'input_0');
    expectInputRemoved(setup);
  });

  it('should shift the later group input sockets while inside the group', () => {
    const setup = createGroupWithTwoInputs();
    setup.manager.enterGroup(setup.groupNode.id);
    setup.manager.removeGroupInput(setup.groupId, 'input_0');
    setup.manager.exitGroup();
    expectInputRemoved(setup);
  });
});

describe('setInputExposed', () => {
  const mockHiddenInputNode: NodeDefinition = {
    id: 'test/node/hidden',
    inputs: {
      value: { type: 'float' },
      strength: { type: 'float', hidden: true }
    },
    outputs: [],
    meta: { title: 'Hidden Input' },
    execute: () => new Int32Array()
  };

  function setup() {
    const registry = createMockNodeRegistry([mockFloatInputNode, mockHiddenInputNode]);
    const manager = new GraphManager(registry);
    const source = manager.createNode({ type: 'test/node/input', position: [0, 0], props: {} });
    const nodeA = manager.createNode({ type: 'test/node/hidden', position: [100, 0], props: {} });
    const nodeB = manager.createNode({ type: 'test/node/hidden', position: [100, 100], props: {} });
    assert.isDefined(source);
    assert.isDefined(nodeA);
    assert.isDefined(nodeB);
    return { manager, source, nodeA, nodeB };
  }

  it('should only show the input on the exposed node', () => {
    const { manager, nodeA, nodeB } = setup();
    manager.setInputExposed(nodeA, 'strength', true);

    expect(manager.getNodeType(nodeA)?.inputs?.strength.hidden).toBe(false);
    expect(manager.getNodeType(nodeB)?.inputs?.strength.hidden).toBe(true);
    expect(getNodeHeight(manager.getNodeType(nodeA)!)).toBeGreaterThan(
      getNodeHeight(manager.getNodeType(nodeB)!)
    );
  });

  it('should only accept edges into exposed inputs', () => {
    const { manager, source, nodeA, nodeB } = setup();
    manager.setInputExposed(nodeA, 'strength', true);

    const sockets = manager.getPossibleSockets({ node: source, index: 0, position: [0, 0] })
      .map(([node, key]) => `${node.id}-${key}`);
    expect(sockets).toContain(`${nodeA.id}-strength`);
    expect(sockets).not.toContain(`${nodeB.id}-strength`);
  });

  it('should save the exposed inputs with the node', () => {
    const { manager, nodeA, nodeB } = setup();
    manager.setInputExposed(nodeA, 'strength', true);

    const graph = manager.serialize();
    expect(graph.nodes.find(n => n.id === nodeA.id)?.exposed).toEqual(['strength']);
    expect(graph.nodes.find(n => n.id === nodeB.id)).not.toHaveProperty('exposed');
  });

  it('should remove edges into an input that is hidden again', () => {
    const { manager, source, nodeA } = setup();
    manager.setInputExposed(nodeA, 'strength', true);
    manager.createEdge(source, 0, nodeA, 'strength');
    manager.createEdge(source, 0, nodeA, 'value');

    manager.setInputExposed(nodeA, 'strength', false);

    expect(manager.serialize().edges).toEqual([[source.id, 0, nodeA.id, 'value']]);
    expect(manager.getNodeType(nodeA)?.inputs?.strength.hidden).toBe(true);
  });

  it('should undo exposing an input', () => {
    const { manager, nodeA } = setup();
    manager.setInputExposed(nodeA, 'strength', true);
    manager.undo();

    expect(manager.getNode(nodeA.id)?.exposed ?? []).toEqual([]);
  });
});

describe('tryConnectToDebugNode', () => {
  function setup() {
    const registry = createMockNodeRegistry([mockFloatOutputNode, mockFloatInputNode, debugNode]);
    const manager = new GraphManager(registry);
    const a = manager.createNode({ type: 'test/node/input', position: [0, 0], props: {} });
    const b = manager.createNode({ type: 'test/node/input', position: [100, 50], props: {} });
    assert.isDefined(a);
    assert.isDefined(b);
    return { manager, a, b };
  }

  it('places the debug node above and to the right of the debugged node', () => {
    const { manager, a } = setup();
    const debug = manager.tryConnectToDebugNode(a.id);
    assert.isDefined(debug);
    expect(debug.position[0]).toBeGreaterThan(a.position[0] + 20);
    expect(debug.position[1] + getNodeHeight(manager.getNodeType(debug)!)).toBeLessThan(
      a.position[1]
    );
    expect(manager.getEdgesToNode(debug).map(e => e[0].id)).toEqual([a.id]);
  });

  it('moves the existing debug node every time, also when it is already connected', () => {
    const { manager, a, b } = setup();
    const debug = manager.tryConnectToDebugNode(a.id);
    assert.isDefined(debug);

    expect(manager.tryConnectToDebugNode(b.id)).toBe(debug);
    expect(debug.position[0]).toBeGreaterThan(b.position[0]);
    expect(manager.getEdgesToNode(debug).map(e => e[0].id)).toEqual([b.id]);

    const before = [...debug.position];
    b.position = [300, 300];
    manager.tryConnectToDebugNode(b.id);
    expect(debug.position).not.toEqual(before);
    expect([...manager.nodes.values()].filter(n => n.type.endsWith('/debug'))).toHaveLength(1);
  });
});
