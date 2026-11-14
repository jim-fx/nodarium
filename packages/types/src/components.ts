import type { Graph, NodeDefinition, NodeId } from './types';

export interface NodeRegistry {
  status: 'loading' | 'ready' | 'error';
  /** Has to be called before getNode or getAllNodes */
  load: (nodeIds: NodeId[]) => Promise<NodeDefinition[]>;
  /**
   * Like load, but a node may come without its code: enough for an editor, not for running it.
   * A later load() of the same ids loads the code.
   */
  loadDefinitions?: (nodeIds: NodeId[]) => Promise<NodeDefinition[]>;
  /** The definitions of every node of a collection, like `max/plantarium`, without their code */
  loadCollection?: (collectionId: `${string}/${string}`) => Promise<NodeDefinition[]>;
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
  /** When the items hold more bytes than this, items are removed */
  maxBytes?: number;
  get: (key: string) => T | undefined;
  /** @param cost the time it took to make the value in ms */
  set: (key: string, value: T, cost?: number) => void;
  /** Whether a value of `bytes` that took `cost` ms to make would be stored, before it is copied */
  keeps?: (bytes: number, cost: number) => boolean;
  /** The items read or written from now until the next call belong to one run */
  startRun?: () => void;
  clear: () => void;
}

export interface AsyncCache<T = unknown> {
  /** When the cache grows beyond this, the oldest items are removed */
  size: number;
  get: <A = T>(key: string) => Promise<A | undefined>;
  set: (key: string, value: T) => Promise<void>;
  clear: () => void;
}
