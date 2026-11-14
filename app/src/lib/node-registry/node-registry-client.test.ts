import type { AsyncCache, NodeId } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { getCollection } from '../node-registry.ts';
import { RemoteNodeRegistry } from './node-registry-client.ts';

const WASM = resolve('static/nodes/max/plantarium/float.wasm');
const BUILT = existsSync(WASM);
const ON_CI = !!process.env.CI;

const float = 'max/plantarium/float' as NodeId;
const floatDefinition = {
  id: float,
  outputs: ['float'],
  inputs: { value: { type: 'float', value: 0.1 } }
};

class MemoryCache implements AsyncCache<ArrayBuffer | string> {
  size = 100;
  values = new Map<string, unknown>();
  writes: string[] = [];
  async get<A>(key: string) {
    return this.values.get(key) as A | undefined;
  }
  async set(key: string, value: ArrayBuffer | string) {
    this.writes.push(key);
    this.values.set(key, value);
  }
  clear() {
    this.values.clear();
  }
}

let wasm: ArrayBuffer;
const requests: string[] = [];

function serve(files: Record<string, () => Response>) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      requests.push(url);
      const file = files[url];
      return file ? file() : new Response('Not found', { status: 404 });
    })
  );
}

const listing = (definitions?: unknown[]) => () =>
  Response.json({ id: 'max/plantarium', nodes: [{ id: float }], definitions });
const wasmFile = () => new Response(wasm.slice(0));

describe.skipIf(!BUILT && !ON_CI)('RemoteNodeRegistry', () => {
  beforeAll(async () => {
    wasm = new Uint8Array(await readFile(WASM)).buffer;
  });

  afterEach(() => {
    requests.length = 0;
    vi.unstubAllGlobals();
  });

  it('takes the definitions from the collection listing without loading wasm', async () => {
    serve({ '/nodes/max/plantarium.json': listing([floatDefinition]) });
    const registry = new RemoteNodeRegistry('');

    const [node] = await registry.loadDefinitions([float]);

    expect(node.id).toBe(float);
    expect(node.execute).toBeUndefined();
    expect(registry.getNode(float)?.inputs?.value).toBeDefined();
    expect(registry.status).toBe('ready');
    expect(requests).toEqual(['/nodes/max/plantarium.json']);
  });

  it('loads the wasm of a node that is only defined when it is loaded to run', async () => {
    serve({
      '/nodes/max/plantarium.json': listing([floatDefinition]),
      '/nodes/max/plantarium/float.wasm': wasmFile
    });
    const registry = new RemoteNodeRegistry('');
    await registry.loadDefinitions([float]);

    const [node] = await registry.load([float]);

    expect(node.execute).toBeTypeOf('function');
    expect(registry.getNode(float)).toBe(node);
    expect(registry.getAllNodes()).toHaveLength(1);
  });

  it('falls back to the wasm when the listing has no definitions', async () => {
    serve({
      '/nodes/max/plantarium.json': listing(),
      '/nodes/max/plantarium/float.wasm': wasmFile
    });
    const registry = new RemoteNodeRegistry('');

    const [node] = await registry.loadDefinitions([float]);

    expect(node.execute).toBeTypeOf('function');
    expect(requests).toContain('/nodes/max/plantarium/float.wasm');
  });

  it('falls back to the wasm when there is no listing', async () => {
    serve({ '/nodes/max/plantarium/float.wasm': wasmFile });
    const registry = new RemoteNodeRegistry('');

    const [node] = await registry.loadDefinitions([float]);

    expect(node.id).toBe(float);
  });

  it('fails like load when neither the listing nor the wasm exist', async () => {
    serve({});
    const registry = new RemoteNodeRegistry('');

    await expect(registry.loadDefinitions(['max/gone/node' as NodeId])).rejects.toThrow(
      'Failed to load nodes/max/gone/node.wasm'
    );
  });

  it('loads a collection with one request', async () => {
    serve({ '/nodes/max/plantarium.json': listing([floatDefinition]) });
    const registry = new RemoteNodeRegistry('');

    await registry.loadDefinitions([float]);
    const nodes = await registry.loadCollection('max/plantarium');

    expect(nodes.map((n) => n.id)).toEqual([float]);
    expect(requests).toEqual(['/nodes/max/plantarium.json']);
  });

  it('does not write unchanged files to the cache again', async () => {
    serve({
      '/nodes/max/plantarium.json': listing([floatDefinition]),
      '/nodes/max/plantarium/float.wasm': wasmFile
    });
    const cache = new MemoryCache();
    await new RemoteNodeRegistry('', cache).load([float]);
    await new RemoteNodeRegistry('', cache).loadDefinitions([float]);
    expect(cache.writes).toEqual([
      '/nodes/max/plantarium/float.wasm',
      '/nodes/max/plantarium.json'
    ]);

    await new RemoteNodeRegistry('', cache).load([float]);
    await new RemoteNodeRegistry('', cache).loadDefinitions([float]);
    await vi.waitFor(() => expect(requests).toHaveLength(4));
    await new Promise((r) => setTimeout(r, 10));

    expect(cache.writes).toHaveLength(2);
  });

  it('writes a changed file to the cache', async () => {
    const cache = new MemoryCache();
    cache.values.set('/nodes/max/plantarium/float.wasm', wasm.slice(0, wasm.byteLength - 1));
    serve({ '/nodes/max/plantarium/float.wasm': wasmFile });

    await new RemoteNodeRegistry('', cache).fetchArrayBuffer('nodes/max/plantarium/float.wasm');
    await vi.waitFor(() => expect(cache.writes).toHaveLength(1));
  });
});

describe.skipIf(!BUILT && !ON_CI)('getCollection', () => {
  it('lists the definitions of the nodes, read from their wasm files', async () => {
    const collection = await getCollection('max/plantarium');

    const definition = collection.definitions.find((d) => d.id === float);
    expect(definition?.outputs).toEqual(['float']);
    expect(collection.definitions).toHaveLength(collection.nodes.length);
  });
});
