import { debugNode } from '@nodarium/runtime';
import type { NodeDefinition } from '@nodarium/types';
import { IndexDBCache } from './node-registry-cache';
import { RemoteNodeRegistry } from './node-registry-client';

export * from './node-registry-cache';
export * from './node-registry-client';

/** The registry of the app: the nodes under /nodes, cached in IndexedDB */
export function createAppRegistry(nodes: NodeDefinition[] = [debugNode]) {
  return new RemoteNodeRegistry('', new IndexDBCache('node-registry'), nodes);
}
