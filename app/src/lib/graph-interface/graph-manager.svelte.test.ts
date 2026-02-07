import { describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import {
  createMockNodeRegistry,
  mockFloatInputNode,
  mockFloatOutputNode,
  mockGeometryOutputNode,
  mockPathInputNode,
  mockVec3OutputNode
} from './test-utils';

describe('GraphManager', () => {
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
});
