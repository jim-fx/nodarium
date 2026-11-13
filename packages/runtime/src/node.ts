import { createRuntime as createRuntimeWith, type RuntimeOptions } from './create-runtime';
import { FileNodeRegistry } from './registry/file-node-registry';

export * from './index';
export { nodesDir } from './nodes-dir';
export { FileNodeRegistry };

/** Like the `createRuntime` of the main entry, reads the nodes of the package by default */
export function createRuntime(options: Partial<RuntimeOptions> = {}) {
  return createRuntimeWith({ ...options, registry: options.registry ?? new FileNodeRegistry() });
}
