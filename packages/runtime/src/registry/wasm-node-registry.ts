import {
  INTERNAL_PREFIX,
  type NodeDefinition,
  NodeDefinitionSchema,
  type NodeId,
  type NodeRegistry
} from '@nodarium/types';
import { createWasmWrapper } from '@nodarium/utils';
import { debugNode } from '../debug-node';

/**
 * Loads every node from its wasm file once and keeps the instance: the runtime calls load() on
 * every execution, and a fresh wasm memory per execution adds up to gigabytes before it is
 * garbage collected.
 */
export abstract class WasmNodeRegistry implements NodeRegistry {
  status: 'loading' | 'ready' | 'error' = 'ready';

  private nodes = new Map<string, NodeDefinition>();
  private pending = new Map<string, Promise<NodeDefinition>>();

  /** @param nodes nodes that have no wasm file, the debug node by default */
  constructor(nodes: NodeDefinition[] = [debugNode]) {
    for (const node of nodes) this.nodes.set(node.id, node);
  }

  /** The bytes of the wasm file of the node `id`, like `max/plantarium/stem` */
  protected abstract readWasm(id: NodeId): Promise<BufferSource>;

  async load(nodeIds: NodeId[]): Promise<NodeDefinition[]> {
    const ids = [...new Set(nodeIds)].filter(id => !id.startsWith(INTERNAL_PREFIX));
    return Promise.all(ids.map(id => {
      const loaded = this.nodes.get(id);
      if (loaded) return loaded;
      let pending = this.pending.get(id);
      if (!pending) {
        pending = this.readWasm(id)
          .then(wasm => this.register(id, wasm as ArrayBuffer))
          .finally(() => this.pending.delete(id));
        this.pending.set(id, pending);
      }
      return pending;
    }));
  }

  async register(id: string, wasmBuffer: ArrayBuffer): Promise<NodeDefinition> {
    const wasm = createWasmWrapper(wasmBuffer);
    const definition = NodeDefinitionSchema.safeParse(wasm.get_definition());
    if (!definition.success) {
      throw new Error(`The definition of the node ${id} is invalid: ${definition.error.message}`);
    }
    const node = { ...definition.data, execute: wasm.execute, reset: wasm.reset };
    this.nodes.set(id, node);
    return node;
  }

  getNode(id: NodeId | string): NodeDefinition | undefined {
    return this.nodes.get(id);
  }

  getAllNodes(): NodeDefinition[] {
    return [...this.nodes.values()];
  }
}
