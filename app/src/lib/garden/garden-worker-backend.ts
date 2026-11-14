import { IndexDBCache, RemoteNodeRegistry } from '#lib/node-registry/index.ts';
import { debugNode, MemoryRuntimeExecutor } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import * as Comlink from 'comlink';

const nodeRegistry = new RemoteNodeRegistry('', new IndexDBCache('node-registry'), [debugNode]);
const executor = new MemoryRuntimeExecutor(nodeRegistry);

/** Executes a project and returns the encoded geometry it produces */
export async function executeGraph(graph: Graph) {
  await nodeRegistry.load(graph.nodes.map((n) => n.type));
  // a project of the garden is built once, its nodes don't need to keep their memory
  const result = await executor.execute(graph, graph.settings ?? {}).finally(() =>
    executor.releaseInstances()
  );
  // the executor keeps using its memory, so a copy is handed out
  const copy = result.slice();
  return Comlink.transfer(copy, [copy.buffer]);
}
