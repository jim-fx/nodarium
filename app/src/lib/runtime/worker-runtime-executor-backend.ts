import { debugNode } from '$lib/node-registry/debugNode';
import { groupInputNode, groupNode, groupOutputNode } from '$lib/node-registry/groupNodes';
import { IndexDBCache, RemoteNodeRegistry } from '$lib/node-registry/index';
import type { Graph } from '@nodarium/types';
import { createPerformanceStore } from '@nodarium/utils';
import { expandGroups, MemoryRuntimeExecutor } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';

const indexDbCache = new IndexDBCache('node-registry');
const nodeRegistry = new RemoteNodeRegistry('', indexDbCache, [
  debugNode,
  groupInputNode,
  groupOutputNode,
  groupNode
]);

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

export async function executeGraph(
  graph: Graph,
  settings: Record<string, unknown>
): Promise<Int32Array> {
  // Expand groups before loading types so we only load real (non-virtual) node types
  const expandedGraph = expandGroups(graph);
  await nodeRegistry.load(
    expandedGraph.nodes
      .map(n => n.type)
      .filter(t => !t.startsWith('__virtual/')) as any
  );
  performanceStore.startRun();
  const res = await executor.execute(graph, settings);
  performanceStore.stopRun();
  return res;
}

export function getPerformanceData() {
  return performanceStore.get();
}

export function getDebugData() {
  return executor.getDebugData();
}
