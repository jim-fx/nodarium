import type { NodeDefinition, NodeId } from '@nodarium/types';
import { WasmNodeRegistry } from './wasm-node-registry';

/** Fetches the nodes from `<baseUrl>/<user>/<collection>/<node>.wasm` */
export class FetchNodeRegistry extends WasmNodeRegistry {
  private baseUrl: string;

  /** @param baseUrl where the `nodes` folder of the package is served, like `/nodes` */
  constructor(baseUrl: string | URL, nodes?: NodeDefinition[]) {
    super(nodes);
    this.baseUrl = String(baseUrl).replace(/\/+$/, '');
  }

  protected async readWasm(id: NodeId): Promise<BufferSource> {
    const url = `${this.baseUrl}/${id}.wasm`;
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`Failed to load the node ${id} from ${url}: ${response.status}`);
    }
    return response.arrayBuffer();
  }
}
