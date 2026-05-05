import { assert, describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
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
