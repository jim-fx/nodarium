import type { NodeDefinition, NodeId, NodeRegistry } from '@nodarium/types';

export function createMockNodeRegistry(nodes: NodeDefinition[]): NodeRegistry {
  const nodesMap = new Map(nodes.map(n => [n.id, n]));
  return {
    status: 'ready' as const,
    load: async (nodeIds: NodeId[]) => {
      const loaded: NodeDefinition[] = [];
      for (const id of nodeIds) {
        if (nodesMap.has(id)) {
          loaded.push(nodesMap.get(id)!);
        }
      }
      return loaded;
    },
    getNode: (id: string) => nodesMap.get(id as NodeId),
    getAllNodes: () => Array.from(nodesMap.values()),
    register: async () => {
      throw new Error('Not implemented in mock');
    }
  };
}

export const mockFloatOutputNode: NodeDefinition = {
  id: 'test/node/output',
  inputs: {},
  outputs: ['float'],
  meta: { title: 'Float Output' },
  execute: () => new Int32Array()
};

export const mockFloatInputNode: NodeDefinition = {
  id: 'test/node/input',
  inputs: { value: { type: 'float' } },
  outputs: [],
  meta: { title: 'Float Input' },
  execute: () => new Int32Array()
};

export const mockGeometryOutputNode: NodeDefinition = {
  id: 'test/node/geometry',
  inputs: {},
  outputs: ['geometry'],
  meta: { title: 'Geometry Output' },
  execute: () => new Int32Array()
};

export const mockPathInputNode: NodeDefinition = {
  id: 'test/node/path',
  inputs: { input: { type: 'path', accepts: ['geometry'] } },
  outputs: [],
  meta: { title: 'Path Input' },
  execute: () => new Int32Array()
};

export const mockVec3OutputNode: NodeDefinition = {
  id: 'test/node/vec3',
  inputs: {},
  outputs: ['vec3'],
  meta: { title: 'Vec3 Output' },
  execute: () => new Int32Array()
};

export const mockIntegerInputNode: NodeDefinition = {
  id: 'test/node/integer',
  inputs: { value: { type: 'integer' } },
  outputs: [],
  meta: { title: 'Integer Input' },
  execute: () => new Int32Array()
};

export const mockBooleanOutputNode: NodeDefinition = {
  id: 'test/node/boolean',
  inputs: {},
  outputs: ['boolean'],
  meta: { title: 'Boolean Output' },
  execute: () => new Int32Array()
};

export const mockBooleanInputNode: NodeDefinition = {
  id: 'test/node/boolean-input',
  inputs: { value: { type: 'boolean' } },
  outputs: [],
  meta: { title: 'Boolean Input' },
  execute: () => new Int32Array()
};
