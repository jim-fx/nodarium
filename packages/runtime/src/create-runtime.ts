import type { NodeRegistry, SyncCache } from '@nodarium/types';
import { MemoryRuntimeExecutor } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';

export type RuntimeOptions = {
  /** where the runtime gets the nodes of a graph from */
  registry: NodeRegistry;
  /**
   * Keeps the results of the nodes, so a run with a changed setting or seed only runs the nodes
   * it changes. Off by default, `true` uses a cache that holds about two runs.
   */
  cache?: boolean | SyncCache<Int32Array[]>;
};

export function createRuntime({ registry, cache }: RuntimeOptions) {
  const runtimeCache = cache === true ? new MemoryRuntimeCache() : cache || undefined;
  return new MemoryRuntimeExecutor(registry, runtimeCache);
}
