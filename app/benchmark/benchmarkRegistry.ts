import { NodeDefinition, NodeId, NodeRegistry } from '@nodarium/types';
import { createWasmWrapper } from '@nodarium/utils';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

export class BenchmarkRegistry implements NodeRegistry {
  status: 'loading' | 'ready' | 'error' = 'loading';

  private nodes = new Map<string, NodeDefinition>();

  async load(nodeIds: NodeId[]): Promise<NodeDefinition[]> {
    // The runtime calls load() on every execution, reuse the instances like
    // the app's registry does, a fresh wasm memory per execution adds up to
    // gigabytes before it is garbage collected
    const nodes = await Promise.all([...new Set(nodeIds)].map(async id => {
      const loaded = this.nodes.get(id);
      if (loaded) return loaded;
      const p = resolve('static/nodes/' + id + '.wasm');
      const file = await readFile(p);
      return this.register(id, file as unknown as ArrayBuffer);
    }));
    this.status = 'ready';
    return nodes;
  }

  async register(id: string, wasmBuffer: ArrayBuffer): Promise<NodeDefinition> {
    const wasm = createWasmWrapper(wasmBuffer);
    const d = wasm.get_definition();
    const node = {
      ...d,
      execute: wasm.execute,
      reset: wasm.reset
    };
    this.nodes.set(id, node);
    return node;
  }

  getNode(id: NodeId | string): NodeDefinition | undefined {
    return this.nodes.get(id);
  }

  getAllNodes(): NodeDefinition[] {
    return [];
  }
}
