import type { Graph, NodeDefinition, NodeId } from './types';

export interface NodeRegistry {
  status: 'loading' | 'ready' | 'error';
  /** Has to be called before getNode or getAllNodes */
  load: (nodeIds: NodeId[]) => Promise<NodeDefinition[]>;
  getNode: (id: NodeId | string) => NodeDefinition | undefined;
  getAllNodes: () => NodeDefinition[];
  register: (id: string, wasmBuffer: ArrayBuffer) => Promise<NodeDefinition>;
}

export interface RuntimeExecutor {
  execute: (
    graph: Graph,
    settings: Record<string, unknown>
  ) => Promise<Int32Array>;
}

export interface SyncCache<T = unknown> {
  /** When the cache grows beyond this, the oldest items are removed */
  size: number;
  get: (key: string) => T | undefined;
  set: (key: string, value: T) => void;
  clear: () => void;
}

export interface AsyncCache<T = unknown> {
  /** When the cache grows beyond this, the oldest items are removed */
  size: number;
  get: <A = T>(key: string) => Promise<A | undefined>;
  set: (key: string, value: T) => Promise<void>;
  clear: () => void;
}
