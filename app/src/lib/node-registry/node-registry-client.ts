import { WasmNodeRegistry } from '@nodarium/runtime';
import {
  type AsyncCache,
  INTERNAL_PREFIX,
  type NodeDefinition,
  NodeDefinitionSchema,
  type NodeId
} from '@nodarium/types';
import { createLogger } from '@nodarium/utils';

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

/**
 * The nodes under `<url>/nodes`, with the listings of their collections. With a cache, a file is
 * answered from it at once and fetched again in the background.
 */
export class RemoteNodeRegistry extends WasmNodeRegistry {
  /** Nodes known from the collection listing, without their code */
  private definitions: Map<string, NodeDefinition> = new Map();
  private collections: Map<string, Promise<CollectionListing>> = new Map();

  constructor(
    private url: string,
    public cache?: AsyncCache<ArrayBuffer | string>,
    nodes: NodeDefinition[] = [],
    /** The wasm files dropped onto the editor, by the id of their node */
    private droppedNodes?: AsyncCache<ArrayBuffer>
  ) {
    super(nodes);
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

  async listCollections(): Promise<`${string}/${string}`[]> {
    const users: { collections: { id: `${string}/${string}` }[] }[] = await this.fetchUsers();
    return users.flatMap((user) => user.collections.map((c) => c.id));
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

  /** Older collection listings name the nodes after their files, `<node>.wasm` */
  private normalizeIds(nodeIds: NodeId[]) {
    return [...new Set(nodeIds.map((id) => id.replace(/\.wasm$/, '') as NodeId))];
  }

  // a node the site serves wins over a dropped file of it, which would hide its updates for good
  protected async readWasm(id: NodeId) {
    try {
      return await this.fetchArrayBuffer(`nodes/${id}.wasm`);
    } catch (error) {
      const dropped = await this.droppedNodes?.get<ArrayBuffer>(id);
      if (dropped) return dropped;
      throw error;
    }
  }

  /** Keeps a dropped wasm file, a project with its node opens again after a reload */
  async keepDropped(node: NodeDefinition, wasm: ArrayBuffer) {
    await this.droppedNodes?.set(node.id, wasm);
  }

  override load(nodeIds: NodeId[]) {
    return super.load(this.normalizeIds(nodeIds));
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
      if (!super.getNode(definition.data.id)) {
        // it has no execute until load() replaces it, which the runtime calls before running it
        this.definitions.set(definition.data.id, definition.data as NodeDefinition);
      }
    }
  }

  override async register(id: string, wasmBuffer: ArrayBuffer) {
    const node = await super.register(id, wasmBuffer);
    this.definitions.delete(node.id);
    return node;
  }

  /** A dropped wasm file comes without an id, every node is found by the id of its definition */
  protected override storageId(_id: string, node: NodeDefinition) {
    return node.id;
  }

  override getNode(id: string) {
    return super.getNode(id) ?? this.definitions.get(id);
  }

  override getAllNodes() {
    return [...super.getAllNodes(), ...this.definitions.values()];
  }
}
