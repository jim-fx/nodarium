import { createAppRegistry } from '#lib/node-registry/index.ts';
import { MemoryRuntimeExecutor } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';
import { loadMatcap, ThumbnailRenderer, type View } from './thumbnail-renderer';

const executor = new MemoryRuntimeExecutor(createAppRegistry());

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

export async function renderViews(result: Int32Array, size: number, views: View[]) {
  const renderer = await getRenderer();
  return renderer.renderViews(result, size, views);
}

export async function renderGraph(graph: Graph, size: number) {
  // a thumbnail is built once, its nodes don't need to keep their memory
  const result = await executor.execute(graph, graph.settings ?? {}).finally(() =>
    executor.releaseInstances()
  );
  return renderResult(result, size);
}
