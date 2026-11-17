import { createAppRegistry } from '#lib/node-registry/index.ts';
import { MemoryRuntimeExecutor } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import * as Comlink from 'comlink';

const executor = new MemoryRuntimeExecutor(createAppRegistry());

/** Executes a project and returns the encoded geometry it produces */
export async function executeGraph(graph: Graph) {
  // a project of the garden is built once, its nodes don't need to keep their memory
  const result = await executor.execute(graph, graph.settings ?? {}).finally(() =>
    executor.releaseInstances()
  );
  // the executor keeps using its memory, so a copy is handed out
  const copy = result.slice();
  return Comlink.transfer(copy, [copy.buffer]);
}
