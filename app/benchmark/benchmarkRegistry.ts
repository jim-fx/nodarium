import { NodeDefinition, NodeId, NodeRegistry } from '@nodarium/types';
import { createWasmWrapper } from '@nodarium/utils';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

export class BenchmarkRegistry implements NodeRegistry {
  status: 'loading' | 'ready' | 'error' = 'loading';

  private nodes = new Map<string, NodeDefinition>();

  async load(nodeIds: NodeId[]): Promise<NodeDefinition[]> {
    const nodes = await Promise.all(nodeIds.map(async id => {
      const p = resolve('static/nodes/' + id + '.wasm');
      const file = await readFile(p);
      const node = createWasmWrapper(file as unknown as ArrayBuffer);
      const d = node.get_definition();
      return {
        ...d,
        execute: node.execute
      };
    }));
    for (const n of nodes) {
      this.nodes.set(n.id, n);
    }
    this.status = 'ready';
    return nodes;
  }

  async register(id: string, wasmBuffer: ArrayBuffer): Promise<NodeDefinition> {
    const wasm = createWasmWrapper(wasmBuffer);
    const d = wasm.get_definition();
    const node = {
      ...d,
      execute: wasm.execute
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
