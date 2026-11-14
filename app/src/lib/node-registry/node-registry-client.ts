import {
  type AsyncCache,
  INTERNAL_PREFIX,
  type NodeDefinition,
  NodeDefinitionSchema,
  type NodeId,
  type NodeRegistry
} from '@nodarium/types';
import { createLogger, createWasmWrapper } from '@nodarium/utils';

const log = createLogger('node-registry');
log.mute();

type CollectionListing = {
  id: string;
  nodes: { id: NodeId }[];
  /** Missing on registries built before the definitions were part of the listing */
  definitions?: unknown[];
};

function sameBytes(a: ArrayBuffer, b: ArrayBuffer) {
  if (a.byteLength !== b.byteLength) return false;
  const x = new Uint8Array(a);
  const y = new Uint8Array(b);
  for (let i = 0; i < x.length; i++) {
    if (x[i] !== y[i]) return false;
  }
  return true;
}

export class RemoteNodeRegistry implements NodeRegistry {
  status: 'loading' | 'ready' | 'error' = 'loading';
  private nodes: Map<string, NodeDefinition> = new Map();
  /** Nodes known from the collection listing, without their code */
  private definitions: Map<string, NodeDefinition> = new Map();
  private pending: Map<string, Promise<NodeDefinition | undefined>> = new Map();
  private collections: Map<string, Promise<CollectionListing>> = new Map();

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

    let cachedValue: string | undefined;
    if (this.cache) {
      cachedValue = await this.cache.get<string>(finalUrl);
      if (cachedValue && !skipCache) {
        this.fetchJson(url, true).catch(() => {});
        return JSON.parse(cachedValue);
      }
    }

    const response = await fetch(finalUrl);

    if (!response.ok) {
      log.error(`Failed to load ${url}`, { response, url, host: this.url });
      throw new Error(`Failed to load ${url}`);
    }

    const text = await response.text();
    if (text !== cachedValue) this.cache?.set(finalUrl, text);

    return JSON.parse(text);
  }

  async fetchArrayBuffer(url: string, skipCache = false) {
    const finalUrl = `${this.url}/${url}`;

    let cachedNode: ArrayBuffer | undefined;
    if (this.cache) {
      cachedNode = await this.cache.get<ArrayBuffer>(finalUrl);
      if (cachedNode && !skipCache) {
        this.fetchArrayBuffer(url, true).catch(() => {});
        return cachedNode;
      }
    }

    const response = await fetch(finalUrl);
    if (!response.ok) {
      log.error(`Failed to load ${url}`, { response, url, host: this.url });
      throw new Error(`Failed to load ${url}`);
    }

    const buffer = await response.arrayBuffer();
    if (!cachedNode || !sameBytes(cachedNode, buffer)) this.cache?.set(finalUrl, buffer);
    return buffer;
  }

  async fetchUsers() {
    return this.fetchJson(`nodes/users.json`);
  }

  async fetchUser(userId: `${string}`) {
    return this.fetchJson(`nodes/${userId}.json`);
  }

  async fetchCollection(userCollectionId: `${string}/${string}`): Promise<CollectionListing> {
    let collection = this.collections.get(userCollectionId);
    if (!collection) {
      collection = this.fetchJson(`nodes/${userCollectionId}.json`) as Promise<CollectionListing>;
      collection.catch(() => this.collections.delete(userCollectionId));
      this.collections.set(userCollectionId, collection);
    }
    return collection;
  }

  async fetchNodeDefinition(nodeId: `${string}/${string}/${string}`) {
    if (nodeId.startsWith(INTERNAL_PREFIX)) return;
    return this.fetchJson(`nodes/${nodeId}.json`);
  }

  private async fetchNodeWasm(nodeId: `${string}/${string}/${string}`) {
    const node = await this.fetchArrayBuffer(`nodes/${nodeId}.wasm`);
    if (!node) {
      throw new Error(`Failed to load node wasm ${nodeId}`);
    }

    return node;
  }

  /** Older collection listings name the nodes after their files, `<node>.wasm` */
  private normalizeIds(nodeIds: NodeId[]) {
    return [...new Set(nodeIds.map((id) => id.replace(/\.wasm$/, '') as NodeId))];
  }

  async load(nodeIds: NodeId[]) {
    const a = performance.now();

    const nodes = (await Promise.all(
      this.normalizeIds(nodeIds).map((id) => {
        if (this.nodes.has(id)) {
          return this.nodes.get(id)!;
        }

        if (id.startsWith(INTERNAL_PREFIX)) return;

        // every node is only instantiated once, even when it is requested again while loading
        let pending = this.pending.get(id);
        if (!pending) {
          pending = this.fetchAndRegister(id).finally(() => this.pending.delete(id));
          this.pending.set(id, pending);
        }
        return pending;
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

  /**
   * The definitions from the listings of their collections, no wasm is loaded. Nodes that a
   * listing has no definition for (a registry built before they were added) are loaded from
   * their wasm files.
   */
  async loadDefinitions(nodeIds: NodeId[]) {
    const ids = this.normalizeIds(nodeIds).filter((id) => !id.startsWith(INTERNAL_PREFIX));

    const byCollection = new Map<`${string}/${string}`, NodeId[]>();
    for (const id of ids) {
      if (this.getNode(id)) continue;
      const [user, collection] = id.split('/');
      const key = `${user}/${collection}` as const;
      byCollection.set(key, [...(byCollection.get(key) ?? []), id]);
    }

    await Promise.all(
      [...byCollection.keys()].map((collection) =>
        this.fetchCollection(collection)
          .then((listing) => this.addDefinitions(listing))
          .catch((error) => log.warn(`No listing for ${collection}`, error))
      )
    );

    const withoutDefinition = ids.filter((id) => !this.getNode(id));
    if (withoutDefinition.length) await this.load(withoutDefinition);

    this.status = 'ready';
    return ids.map((id) => this.getNode(id)).filter(Boolean) as NodeDefinition[];
  }

  async loadCollection(collectionId: `${string}/${string}`) {
    const listing = await this.fetchCollection(collectionId);
    return this.loadDefinitions(listing.nodes.map((n) => n.id));
  }

  private addDefinitions(listing: CollectionListing) {
    if (!Array.isArray(listing.definitions)) return;
    for (const raw of listing.definitions) {
      const definition = NodeDefinitionSchema.safeParse(raw);
      if (!definition.success) {
        log.warn('Invalid definition in the listing', raw, definition.error);
        continue;
      }
      if (!this.nodes.has(definition.data.id)) {
        // it has no execute until load() replaces it, which the runtime calls before running it
        this.definitions.set(definition.data.id, definition.data as NodeDefinition);
      }
    }
  }

  private async fetchAndRegister(id: `${string}/${string}/${string}`) {
    const wasmBuffer = await this.fetchNodeWasm(id);

    try {
      return await this.register(id, wasmBuffer);
    } catch (e) {
      console.log('Failed to register: ', id);
      console.error(e);
      return;
    }
  }

  async register(id: string, wasmBuffer: ArrayBuffer) {
    const wrapper = (() => {
      try {
        return createWasmWrapper(wasmBuffer);
      } catch (error) {
        console.error(`Failed to create node wrapper for node: ${id}`, error);
        throw error;
      }
    })();

    const rawDefinition = wrapper.get_definition();
    const definition = NodeDefinitionSchema.safeParse(rawDefinition);

    if (definition.error) {
      throw new Error(
        'Failed to parse node definition from node:+\n' + JSON.stringify(rawDefinition, null, 2)
          + '\n'
          + definition.error
      );
    }

    const node = {
      ...definition.data,
      execute: wrapper.execute,
      reset: wrapper.reset,
      release: wrapper.release,
      memoryBytes: wrapper.memoryBytes
    };

    this.nodes.set(definition.data.id, node);
    this.definitions.delete(definition.data.id);

    return node;
  }

  getNode(id: string) {
    return this.nodes.get(id) ?? this.definitions.get(id);
  }

  getAllNodes() {
    return [...this.nodes.values(), ...this.definitions.values()];
  }
}
