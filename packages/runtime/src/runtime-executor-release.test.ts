import type { Graph, NodeDefinition, NodeRegistry, NodeValue } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FileNodeRegistry } from './registry/file-node-registry';
import { MemoryRuntimeExecutor } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';
import { createMockNodeRegistry } from './test-utils';

const MB = 2 ** 20;

// stands in for a wasm node: `bytes` is its memory, 0 once it is released
function fakeWasmNode(id: string, bytes: number) {
  const node = {
    id,
    inputs: { input: { type: 'integer', value: 0 } },
    outputs: ['float'],
    memory: 0,
    execute: vi.fn((values: NodeValue[]) => {
      node.memory = bytes;
      return Int32Array.of(((values[0] as Int32Array | undefined)?.[0] ?? 0) + 1);
    }),
    reset: vi.fn(),
    release: vi.fn(() => {
      node.memory = 0;
    }),
    memoryBytes: () => node.memory
  };
  return node;
}

let output: ReturnType<typeof fakeWasmNode>;
let small: ReturnType<typeof fakeWasmNode>;
let other: ReturnType<typeof fakeWasmNode>;
let big: ReturnType<typeof fakeWasmNode>;

function chain(...types: string[]): Graph {
  const nodes = [output.id, ...types].map((type, id) => ({ id, type, position: [0, 0] }));
  const edges = types.map((_, i) => [i + 1, 0, i, 'input']);
  return { id: 0, nodes, edges, groups: [] } as unknown as Graph;
}

function createExecutor(cache = false) {
  const registry = createMockNodeRegistry(
    [output, small, other, big] as unknown as NodeDefinition[]
  );
  return new MemoryRuntimeExecutor(registry, cache ? new MemoryRuntimeCache() : undefined);
}

describe('releasing instances', () => {
  beforeEach(() => {
    output = fakeWasmNode('test/release/output', MB);
    small = fakeWasmNode('test/release/small', MB);
    other = fakeWasmNode('test/release/other', MB);
    big = fakeWasmNode('test/release/big', 64 * MB);
  });

  it('keeps the instances of the nodes of the graph', async () => {
    const executor = createExecutor();
    await executor.execute(chain(small.id), {});
    await executor.execute(chain(small.id), {});
    expect(small.release).not.toHaveBeenCalled();
    expect(output.release).not.toHaveBeenCalled();
    expect(small.reset).toHaveBeenCalledTimes(2);
  });

  it('releases the nodes that left the graph after the run', async () => {
    const executor = createExecutor();
    await executor.execute(chain(small.id, other.id), {});
    await executor.execute(chain(small.id), {});
    expect(other.release).toHaveBeenCalledTimes(1);
    expect(small.release).not.toHaveBeenCalled();

    // a released node has nothing to reset any more
    await executor.execute(chain(small.id), {});
    expect(other.release).toHaveBeenCalledTimes(1);
    expect(other.reset).toHaveBeenCalledTimes(2);
  });

  it('releases the nodes with more memory than releaseInstancesOver after every run', async () => {
    const executor = createExecutor();
    expect(await executor.execute(chain(big.id, small.id), {})).toEqual(Int32Array.of(3));
    expect(big.release).toHaveBeenCalledTimes(1);
    expect(small.release).not.toHaveBeenCalled();

    expect(await executor.execute(chain(big.id, small.id), {})).toEqual(Int32Array.of(3));
    expect(big.release).toHaveBeenCalledTimes(2);

    executor.releaseInstancesOver = Infinity;
    await executor.execute(chain(big.id, small.id), {});
    expect(big.release).toHaveBeenCalledTimes(2);
  });

  it('keeps the cache entries of a released node', async () => {
    const executor = createExecutor(true);
    await executor.execute(chain(big.id, small.id), {});
    await executor.execute(chain(big.id, small.id), {});
    expect(big.release).toHaveBeenCalledTimes(1);
    expect(big.execute).toHaveBeenCalledTimes(1);
  });

  it('releases after a failed run as well', async () => {
    const executor = createExecutor();
    big.execute.mockImplementationOnce(() => {
      big.memory = 64 * MB;
      throw new Error('panic');
    });
    await expect(executor.execute(chain(big.id), {})).rejects.toThrow();
    expect(big.release).toHaveBeenCalledTimes(1);
  });

  it('waits for overlapping runs before releasing', async () => {
    const registry = createMockNodeRegistry(
      [output, small, other, big] as unknown as NodeDefinition[]
    );
    let resume = () => {};
    const gated: NodeRegistry = {
      ...registry,
      load: async (ids) => {
        if (ids.some((id) => id === other.id)) await new Promise<void>((r) => (resume = r));
        return registry.load(ids);
      }
    };
    const executor = new MemoryRuntimeExecutor(gated);
    await executor.execute(chain(small.id), {});

    // the second run still loads its nodes when the first one, without `small`, finishes
    const second = executor.execute(chain(small.id, other.id), {});
    await executor.execute(chain(big.id), {});
    expect(small.release).not.toHaveBeenCalled();
    expect(big.release).not.toHaveBeenCalled();

    resume();
    expect(await second).toEqual(Int32Array.of(3));
    expect(small.release).not.toHaveBeenCalled();
    expect(big.release).toHaveBeenCalledTimes(1);
  });

  it('releaseInstances() during a run releases after it', async () => {
    const registry = createMockNodeRegistry(
      [output, small, other, big] as unknown as NodeDefinition[]
    );
    let resume = () => {};
    const gated: NodeRegistry = {
      ...registry,
      load: async (ids) => {
        await new Promise<void>((r) => (resume = r));
        return registry.load(ids);
      }
    };
    const executor = new MemoryRuntimeExecutor(gated);
    const first = executor.execute(chain(small.id), {});
    resume();
    await first;

    const run = executor.execute(chain(small.id), {});
    executor.releaseInstances();
    expect(small.release).not.toHaveBeenCalled();
    resume();
    await run;
    expect(small.release).toHaveBeenCalledTimes(1);
  });

  it('releaseInstances() releases every node that ran', async () => {
    const executor = createExecutor();
    await executor.execute(chain(small.id), {});
    executor.releaseInstances();
    expect(small.release).toHaveBeenCalledTimes(1);
    expect(output.release).toHaveBeenCalledTimes(1);
    expect(other.release).not.toHaveBeenCalled();
  });
});

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
const dir = fileURLToPath(new URL('../../../app/static/nodes/', import.meta.url));
const BUILT = existsSync(`${dir}max/plantarium/stem.wasm`);

describe.skipIf(!BUILT && !ON_CI)('releasing instances of compiled nodes', () => {
  const graph = {
    id: 0,
    nodes: [
      { id: 0, type: 'max/plantarium/output', position: [0, 0] },
      { id: 1, type: 'max/plantarium/stem', position: [0, 0] }
    ],
    edges: [[1, 0, 0, 'input']],
    groups: []
  } as unknown as Graph;

  it('instantiates a released node again on its next run, with the same result', async () => {
    const registry = new FileNodeRegistry(dir, []);
    const executor = new MemoryRuntimeExecutor(registry);
    const first = await executor.execute(graph, {});
    const stem = registry.getNode('max/plantarium/stem')!;
    expect(stem.memoryBytes!()).toBeGreaterThan(0);

    executor.releaseInstancesOver = 0;
    const second = await executor.execute(graph, {});
    expect(stem.memoryBytes!()).toBe(0);
    expect(registry.getNode('max/plantarium/output')!.memoryBytes!()).toBe(0);

    const third = await executor.execute(graph, {});
    expect(first.length).toBeGreaterThan(0);
    expect(second).toEqual(first);
    expect(third).toEqual(first);
  });

  it('releaseInstances() frees the memory of every node', async () => {
    const registry = new FileNodeRegistry(dir, []);
    const executor = new MemoryRuntimeExecutor(registry);
    const first = await executor.execute(graph, {});
    executor.releaseInstances();
    for (const node of registry.getAllNodes()) expect(node.memoryBytes!()).toBe(0);
    expect(await executor.execute(graph, {})).toEqual(first);
  });
});
