import { IndexDBCache, RemoteNodeRegistry } from '#lib/node-registry/index.ts';
import { debugNode, MemoryRuntimeExecutor } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import { loadMatcap, ThumbnailRenderer } from './thumbnail-renderer';

const nodeRegistry = new RemoteNodeRegistry('', new IndexDBCache('node-registry'), [debugNode]);
const executor = new MemoryRuntimeExecutor(nodeRegistry);

let rendererPromise: Promise<ThumbnailRenderer> | undefined;
function getRenderer() {
  rendererPromise ??= loadMatcap('/matcap_green.jpg').then((matcap) =>
    new ThumbnailRenderer(matcap)
  );
  return rendererPromise;
}

export async function renderResult(result: Int32Array, size: number) {
  const renderer = await getRenderer();
  return renderer.render(result, size);
}

export async function renderGraph(graph: Graph, size: number) {
  await nodeRegistry.load(graph.nodes.map((n) => n.type));
  // a thumbnail is built once, its nodes don't need to keep their memory
  const result = await executor.execute(graph, graph.settings ?? {}).finally(() =>
    executor.releaseInstances()
  );
  return renderResult(result, size);
}
