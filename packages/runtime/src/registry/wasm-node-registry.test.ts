import type { NodeDefinition, NodeId } from '@nodarium/types';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WasmNodeRegistry } from './wasm-node-registry';

const wrapper = vi.hoisted(() => ({ definition: undefined as unknown }));

vi.mock('@nodarium/utils', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@nodarium/utils')>();
  return {
    ...actual,
    createWasmWrapper: (buffer: ArrayBuffer) => {
      const real = actual.createWasmWrapper(buffer);
      return wrapper.definition === undefined
        ? real
        : { ...real, get_definition: () => wrapper.definition };
    }
  };
});

const wasmPath = fileURLToPath(
  new URL('../../../../app/static/nodes/max/core/float.wasm', import.meta.url)
);

class FakeRegistry extends WasmNodeRegistry {
  reads: NodeId[] = [];
  failNext = false;

  constructor(private wasm: ArrayBuffer) {
    super([]);
  }

  protected async readWasm(id: NodeId): Promise<BufferSource> {
    this.reads.push(id);
    await Promise.resolve();
    if (this.failNext) {
      this.failNext = false;
      throw new Error('network down');
    }
    return this.wasm;
  }
}

const id = 'max/core/float' as NodeId;

describe('WasmNodeRegistry', () => {
  let registry: FakeRegistry;

  beforeEach(async () => {
    wrapper.definition = undefined;
    registry = new FakeRegistry(new Uint8Array(await readFile(wasmPath)).buffer);
  });

  it('loads a node once for concurrent loads of the same id', async () => {
    const [a, b] = await Promise.all([registry.load([id]), registry.load([id, id])]);
    expect(registry.reads).toEqual([id]);
    expect(a[0]).toBe(b[0]);
    expect(a[0].id).toBe(id);
  });

  it('keeps the loaded node', async () => {
    await registry.load([id]);
    await registry.load([id]);
    expect(registry.reads).toHaveLength(1);
    expect(registry.getNode(id)).toBeDefined();
  });

  it('does not cache a failed load, a retry reads again', async () => {
    registry.failNext = true;
    await expect(registry.load([id])).rejects.toThrow('network down');
    expect(registry.getNode(id)).toBeUndefined();

    const [node] = await registry.load([id]);
    expect(registry.reads).toEqual([id, id]);
    expect(node.id).toBe(id);
  });

  it('rejects an invalid definition with the node id and does not keep it', async () => {
    wrapper.definition = { id, outputs: 'not a list' };
    await expect(registry.load([id])).rejects.toThrow(
      `The definition of the node ${id} is invalid`
    );
    expect(registry.getNode(id)).toBeUndefined();

    wrapper.definition = undefined;
    await expect(registry.load([id])).resolves.toHaveLength(1);
    expect(registry.reads).toHaveLength(2);
  });

  it('stores a registered node under the id it was given', async () => {
    const wasm = new Uint8Array(await readFile(wasmPath)).buffer;
    const node = await registry.register('my/own/float', wasm);

    expect(node.id).toBe(id);
    expect(registry.getNode('my/own/float')).toBe(node);
    expect(registry.getNode(id)).toBeUndefined();
  });

  it('lets a subclass store a node under the id of its definition', async () => {
    class ByDefinition extends FakeRegistry {
      protected storageId(_id: string, node: NodeDefinition) {
        return node.id;
      }
    }
    const wasm = new Uint8Array(await readFile(wasmPath)).buffer;
    const byDefinition = new ByDefinition(wasm);

    const node = await byDefinition.register('', wasm);

    expect(byDefinition.getNode(id)).toBe(node);
    expect(byDefinition.getNode('')).toBeUndefined();
    expect(byDefinition.getAllNodes()).toEqual([node]);
  });
});
