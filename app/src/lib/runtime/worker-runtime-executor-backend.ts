import { IndexDBCache, RemoteNodeRegistry } from '#lib/node-registry/index.ts';
import { debugNode, MemoryRuntimeCache, MemoryRuntimeExecutor } from '@nodarium/runtime';
import type { Graph, NodeId } from '@nodarium/types';
import { createPerformanceStore } from '@nodarium/utils';
import * as Comlink from 'comlink';
import type { WorkerExecution } from './types';

const indexDbCache = new IndexDBCache('node-registry');
const nodeRegistry = new RemoteNodeRegistry('', indexDbCache, [debugNode]);

const cache = new MemoryRuntimeCache();
const executor = new MemoryRuntimeExecutor(nodeRegistry, cache);

const performanceStore = createPerformanceStore();
executor.perf = performanceStore;

export async function setUseRegistryCache(useCache: boolean) {
  if (useCache) {
    nodeRegistry.cache = indexDbCache;
  } else {
    nodeRegistry.cache = undefined;
  }
}

export async function setUseRuntimeCache(useCache: boolean) {
  if (useCache) {
    executor.cache = cache;
  } else {
    executor.cache = undefined;
  }
}

/** Starts loading the nodes of a graph while the editor is still reading their definitions */
export async function loadNodes(nodeIds: NodeId[]) {
  await nodeRegistry.load(nodeIds).catch(() => {});
}

/**
 * Everything the app reads after a run comes back with the result, every extra call would be
 * another round trip to the worker
 */
export async function executeGraph(
  graph: Graph,
  settings: Record<string, unknown>
): Promise<WorkerExecution> {
  await nodeRegistry.load(graph.nodes.map((n) => n.type));
  performanceStore.startRun();
  const result = await executor.execute(graph, settings);
  performanceStore.stopRun();
  const execution: WorkerExecution = {
    result,
    performance: performanceStore.get().at(-1),
    // debug values can be entries of the runtime cache, so they are copied, not transferred
    debugData: executor.getDebugData(),
    nodeTimings: executor.getNodeTimings(),
    runTimings: executor.getRunTimings()
  };
  if (result?.buffer) {
    return Comlink.transfer(execution, [result.buffer]);
  }
  return execution;
}

export function getLastError() {
  return executor.getLastError();
}
