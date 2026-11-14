/// <reference types="vite-plugin-comlink/client" />
import type { Graph, NodeId, RuntimeExecutor } from '@nodarium/types';
import type { WorkerExecution } from './types';

export class WorkerRuntimeExecutor implements RuntimeExecutor {
  private worker = new ComlinkWorker<typeof import('./worker-runtime-executor-backend.ts')>(
    new URL(`./worker-runtime-executor-backend.ts`, import.meta.url)
  );

  // the details of the last run arrive together with its result
  private last?: Omit<WorkerExecution, 'result'>;

  async execute(graph: Graph, settings: Record<string, unknown>) {
    this.last = undefined;
    const { result, ...details } = await this.worker.executeGraph(graph, settings);
    this.last = details;
    return result;
  }
  /** The worker fetches and compiles the nodes now instead of on the first run */
  loadNodes(nodeIds: NodeId[]) {
    return this.worker.loadNodes(nodeIds);
  }
  /** The timings of the last run */
  getPerformanceData() {
    return this.last?.performance;
  }
  getDebugData() {
    return this.last?.debugData ?? {};
  }
  getNodeTimings() {
    return this.last?.nodeTimings ?? {};
  }
  getRunTimings() {
    return this.last?.runTimings ?? {};
  }
  async getLastError() {
    return await this.worker.getLastError();
  }
  set useRuntimeCache(useCache: boolean) {
    this.worker.setUseRuntimeCache(useCache);
  }
  set useRegistryCache(useCache: boolean) {
    this.worker.setUseRegistryCache(useCache);
  }
}
