import {
  type AsyncCache,
  type NodeDefinition,
  NodeDefinitionSchema,
  type NodeId,
  type NodeRegistry
} from '@nodarium/types';
import { createLogger, createWasmWrapper } from '@nodarium/utils';

const log = createLogger('node-registry');
log.mute();

export class RemoteNodeRegistry implements NodeRegistry {
  status: 'loading' | 'ready' | 'error' = 'loading';
  private nodes: Map<string, NodeDefinition> = new Map();

  constructor(
    private url: string,
    public cache?: AsyncCache<ArrayBuffer | string>,
    nodes?: NodeDefinition[]
  ) {
    if (nodes?.length) {
      for (const node of nodes) {
        this.nodes.set(node.id, node);
      }
    }
  }

  async fetchJson(url: string, skipCache = false) {
    const finalUrl = `${this.url}/${url}`;

    if (!skipCache && this.cache) {
      const cachedValue = await this.cache?.get<string>(finalUrl);
      if (cachedValue) {
        // fetch again in the background, maybe implement that only refetch after a certain time
        this.fetchJson(url, true);
        return JSON.parse(cachedValue);
      }
    }

    const response = await fetch(finalUrl);

    if (!response.ok) {
      log.error(`Failed to load ${url}`, { response, url, host: this.url });
      throw new Error(`Failed to load ${url}`);
    }

    const result = await response.json();

    this.cache?.set(finalUrl, JSON.stringify(result));

    return result;
  }

  async fetchArrayBuffer(url: string, skipCache = false) {
    const finalUrl = `${this.url}/${url}`;

    if (!skipCache && this.cache) {
      const cachedNode = await this.cache?.get<ArrayBuffer>(finalUrl);
      if (cachedNode) {
        // fetch again in the background, maybe implement that only refetch after a certain time
        this.fetchArrayBuffer(url, true);
        return cachedNode;
      }
    }

    const response = await fetch(finalUrl);
    if (!response.ok) {
      log.error(`Failed to load ${url}`, { response, url, host: this.url });
      throw new Error(`Failed to load ${url}`);
    }

    const buffer = await response.arrayBuffer();
    this.cache?.set(finalUrl, buffer);
    return buffer;
  }

  async fetchUsers() {
    return this.fetchJson(`nodes/users.json`);
  }

  async fetchUser(userId: `${string}`) {
    return this.fetchJson(`user/${userId}.json`);
  }

  async fetchCollection(userCollectionId: `${string}/${string}`) {
    const col = await this.fetchJson(`nodes/${userCollectionId}.json`);
    return col;
  }

  async fetchNodeDefinition(nodeId: `${string}/${string}/${string}`) {
    return this.fetchJson(`nodes/${nodeId}.json`);
  }

  private async fetchNodeWasm(nodeId: `${string}/${string}/${string}`) {
    const node = await this.fetchArrayBuffer(`nodes/${nodeId}.wasm`);
    if (!node) {
      throw new Error(`Failed to load node wasm ${nodeId}`);
    }

    return node;
  }

  async load(nodeIds: `${string}/${string}/${string}`[]) {
    const a = performance.now();

    const nodes = (await Promise.all(
      [...new Set(nodeIds).values()].map(async (id) => {
        if (this.nodes.has(id)) {
          return this.nodes.get(id)!;
        }

        const wasmBuffer = await this.fetchNodeWasm(id);

        try {
          return await this.register(id, wasmBuffer);
        } catch (e) {
          console.log('Failed to register: ', id);
          console.error(e);
          return;
        }
      })
    )).filter(Boolean) as NodeDefinition[];

    const duration = performance.now() - a;

    log.group('loaded nodes in', duration, 'ms');
    log.info(nodeIds);
    log.info(nodes);
    log.groupEnd();
    this.status = 'ready';

    return nodes;
  }

  async register(id: string, wasmBuffer: ArrayBuffer) {
    let wrapper: ReturnType<typeof createWasmWrapper> = null!;
    try {
      wrapper = createWasmWrapper(wasmBuffer);
    } catch (error) {
      console.error(`Failed to create node wrapper for node: ${id}`, error);
    }

    const rawDefinition = wrapper.get_definition();
    const definition = NodeDefinitionSchema.safeParse(rawDefinition);

    if (definition.error) {
      throw new Error(
        'Failed to parse node definition from node:+\n' + JSON.stringify(rawDefinition, null, 2)
          + '\n'
          + definition.error
      );
    }

    if (this.cache) {
      this.cache.set(definition.data.id, wasmBuffer);
    }

    const node = {
      ...definition.data,
      execute: wrapper.execute
    };

    this.nodes.set(definition.data.id, node);

    return node;
  }

  getNode(id: string) {
    return this.nodes.get(id);
  }

  getAllNodes() {
    const allNodes = [...this.nodes.values()];
    log.info('getting all nodes', allNodes);
    return allNodes;
  }

  async overwriteNode(nodeId: NodeId, node: NodeDefinition) {
    log.info('Overwritten node', { nodeId, node });
    this.nodes.set(nodeId, node);
  }
}
