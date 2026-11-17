import { createRuntime as createRuntimeWith, type RuntimeOptions } from './create-runtime';
import { FileNodeRegistry } from './registry/file-node-registry';

export {
  findNodes,
  firstSentence,
  formatNode,
  formatNodeList,
  nodeSignature,
  nodesOfType,
  nodeSummary
} from './catalog';
export {
  applySets,
  type GraphSets,
  type GraphSource,
  parseSets,
  readGraphSource
} from './cli/graph-input';
export * from './index';
export { listNodeIds, listNodes } from './list-nodes';
export { nodesDir } from './nodes-dir';
export {
  DEFAULT_VIEWS,
  type PlacedCell,
  RENDER_VIEWS,
  type RenderedSheet,
  type RenderOptions,
  renderSheetToPng,
  type RenderStats,
  renderToPng,
  type RenderView,
  type SheetCell,
  type SheetOptions
} from './render';
export { FileNodeRegistry };

/** Like the `createRuntime` of the main entry, reads the nodes of the package by default */
export function createRuntime(options: Partial<RuntimeOptions> = {}) {
  return createRuntimeWith({ ...options, registry: options.registry ?? new FileNodeRegistry() });
}
