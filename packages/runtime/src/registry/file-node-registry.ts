import type { NodeDefinition, NodeId } from '@nodarium/types';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { nodesDir } from '../nodes-dir';
import { WasmNodeRegistry } from './wasm-node-registry';

/** Reads the nodes from `<dir>/<user>/<collection>/<node>.wasm`, Node only */
export class FileNodeRegistry extends WasmNodeRegistry {
  /** @param dir the nodes that come with the package by default */
  constructor(private dir: string = nodesDir, nodes?: NodeDefinition[]) {
    super(nodes);
  }

  protected async readWasm(id: NodeId): Promise<BufferSource> {
    return readFile(join(this.dir, `${id}.wasm`));
  }
}
