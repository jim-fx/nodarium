import { createRuntime as createRuntimeWith, type RuntimeOptions } from './create-runtime';
import { FileNodeRegistry } from './registry/file-node-registry';

export {
  findNodes,
  firstSentence,
  formatNode,
  formatNodeList,
  nodeSignature,
  nodesOfType
} from './catalog';
export * from './index';
export { listNodeIds, listNodes } from './list-nodes';
export { nodesDir } from './nodes-dir';
export { type RenderOptions, type RenderStats, renderToPng } from './render';
export { FileNodeRegistry };

/** Like the `createRuntime` of the main entry, reads the nodes of the package by default */
export function createRuntime(options: Partial<RuntimeOptions> = {}) {
  return createRuntimeWith({ ...options, registry: options.registry ?? new FileNodeRegistry() });
}
