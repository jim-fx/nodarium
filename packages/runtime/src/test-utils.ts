import type { NodeDefinition, NodeId, NodeRegistry } from '@nodarium/types';

export function createMockNodeRegistry(nodes: NodeDefinition[]): NodeRegistry {
  const nodesMap = new Map(nodes.map(n => [n.id, n]));
  return {
    status: 'ready' as const,
    load: async (nodeIds: NodeId[]) => nodeIds.flatMap(id => nodesMap.get(id) ?? []),
    getNode: (id: string) => nodesMap.get(id as NodeId),
    getAllNodes: () => Array.from(nodesMap.values()),
    register: async () => {
      throw new Error('Not implemented in mock');
    }
  };
}
