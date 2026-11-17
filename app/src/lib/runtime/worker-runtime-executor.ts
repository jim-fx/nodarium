/// <reference types="vite-plugin-comlink/client" />
import type { Graph, NodeId } from '@nodarium/types';
import { unwrapExecution } from './run-error';
import type { AppRuntime } from './types';

export class WorkerRuntimeExecutor implements AppRuntime {
  private worker = new ComlinkWorker<typeof import('./worker-runtime-executor-backend.ts')>(
    new URL(`./worker-runtime-executor-backend.ts`, import.meta.url)
  );

  async run(graph: Graph, settings: Record<string, unknown>) {
    return unwrapExecution(await this.worker.executeGraph(graph, settings));
  }
  /** The worker fetches and compiles the nodes now instead of on the first run */
  loadNodes(nodeIds: NodeId[]) {
    return this.worker.loadNodes(nodeIds);
  }
  registerNode(wasm: ArrayBuffer) {
    return this.worker.registerNode(wasm);
  }
  set useRuntimeCache(useCache: boolean) {
    this.worker.setUseRuntimeCache(useCache);
  }
  set useRegistryCache(useCache: boolean) {
    this.worker.setUseRegistryCache(useCache);
  }
}
