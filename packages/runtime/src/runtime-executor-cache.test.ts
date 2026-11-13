import type { Graph, NodeDefinition, NodeValue } from '@nodarium/types';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRuntimeExecutor, toCreatorId } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';
import { createMockNodeRegistry } from './test-utils';

// Every node sums its inputs, so results depend on everything the cache key
// has to cover. `calls` counts how often each node type really executed.
let calls: Record<string, number> = {};

function sumNode(id: string, inputs: NodeDefinition['inputs']): NodeDefinition {
  return {
    id: id as NodeDefinition['id'],
    inputs,
    outputs: ['float'],
    execute: vi.fn((values: NodeValue[]) => {
      calls[id] = (calls[id] ?? 0) + 1;
      let sum = 0;
      for (const value of values) sum += (value as Int32Array)[0];
      return Int32Array.of(sum);
    }),
    reset: vi.fn()
  } as NodeDefinition;
}

const output = sumNode('test/cache/output', { input: { type: 'float' } });
const add = sumNode('test/cache/add', { a: { type: 'integer' }, b: { type: 'integer' } });
const left = sumNode('test/cache/left', { value: { type: 'integer', value: 1 } });
const right = sumNode('test/cache/right', { value: { type: 'integer', value: 2 } });
const seeded = sumNode('test/cache/seeded', { seed: { type: 'seed' } });
const creator = sumNode('test/cache/creator', { creator: { type: 'creator' } });
const setting = sumNode('test/cache/setting', {
  value: { type: 'integer', setting: 'resolution' }
});

const BIG_BYTES = 4096;
// a result of BIG_BYTES filled with the seed
const bigSeeded = {
  id: 'test/cache/big-seeded',
  inputs: { seed: { type: 'seed' } },
  outputs: ['float'],
  execute: vi.fn((values: NodeValue[]) => {
    calls['test/cache/big-seeded'] = (calls['test/cache/big-seeded'] ?? 0) + 1;
    return new Int32Array(BIG_BYTES / 4).fill((values[0] as Int32Array)[0]);
  }),
  reset: vi.fn()
} as NodeDefinition;

const registry = createMockNodeRegistry([
  output,
  add,
  left,
  right,
  seeded,
  creator,
  setting,
  bigSeeded
]);

// output <- add <- (left, right)
function graph(leftProps?: Record<string, number>): Graph {
  return {
    id: 0,
    nodes: [
      { id: 0, type: output.id, position: [0, 0] },
      { id: 1, type: add.id, position: [0, 0] },
      { id: 2, type: left.id, position: [0, 0], ...(leftProps ? { props: leftProps } : {}) },
      { id: 3, type: right.id, position: [0, 0] }
    ],
    edges: [
      [1, 0, 0, 'input'],
      [2, 0, 1, 'a'],
      [3, 0, 1, 'b']
    ]
  } as Graph;
}

function singleNodeGraph(type: string): Graph {
  return {
    id: 0,
    nodes: [
      { id: 0, type: output.id, position: [0, 0] },
      { id: 1, type, position: [0, 0] }
    ],
    edges: [[1, 0, 0, 'input']]
  } as Graph;
}

function createExecutor() {
  return new MemoryRuntimeExecutor(registry, new MemoryRuntimeCache());
}

describe('runtime cache keys', () => {
  beforeEach(() => {
    calls = {};
  });

  it('serves an unchanged graph from the cache, except the output node', async () => {
    const executor = createExecutor();
    const first = await executor.execute(graph(), {});
    calls = {};

    const second = await executor.execute(graph(), {});

    expect(second).toEqual(first);
    expect(calls).toEqual({ [output.id]: 1 });
  });

  it('re-runs only the changed node and the nodes that depend on it', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    calls = {};

    const result = await executor.execute(graph({ value: 5 }), {});

    expect(result).toEqual(Int32Array.of(7));
    expect(calls).toEqual({ [left.id]: 1, [add.id]: 1, [output.id]: 1 });
  });

  it('uses the default value of an unconnected input in the key', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    calls = {};

    // the same value as the default, so the result can't change
    await executor.execute(graph({ value: 1 }), {});

    expect(calls).toEqual({ [output.id]: 1 });
  });

  it('returns to the cached result when a change is undone', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    await executor.execute(graph({ value: 5 }), {});
    calls = {};

    const result = await executor.execute(graph(), {});

    expect(result).toEqual(Int32Array.of(3));
    expect(calls).toEqual({ [output.id]: 1 });
  });

  it('re-runs nodes that read a changed setting', async () => {
    const executor = createExecutor();
    await executor.execute(singleNodeGraph(setting.id), { resolution: 4 });
    calls = {};

    await executor.execute(singleNodeGraph(setting.id), { resolution: 4 });
    expect(calls).toEqual({ [output.id]: 1 });

    const result = await executor.execute(singleNodeGraph(setting.id), { resolution: 8 });
    expect(result).toEqual(Int32Array.of(8));
    expect(calls[setting.id]).toBe(1);
  });

  it('keeps the nodes beside a changed setting cached', async () => {
    // like time -> expand <- gills, a change of the time must not re-run gills
    const timed = (resolution: number) => {
      const g = graph();
      g.nodes[3] = { id: 3, type: setting.id, position: [0, 0] } as Graph['nodes'][number];
      return [g, { resolution }] as const;
    };
    const executor = createExecutor();
    await executor.execute(...timed(4));
    calls = {};

    const result = await executor.execute(...timed(8));

    expect(result).toEqual(Int32Array.of(9));
    expect(calls).toEqual({ [setting.id]: 1, [add.id]: 1, [output.id]: 1 });
  });

  it('re-runs seeded nodes when the seed changes', async () => {
    const executor = createExecutor();
    await executor.execute(singleNodeGraph(seeded.id), { seed: 1 });
    calls = {};

    await executor.execute(singleNodeGraph(seeded.id), { seed: 1 });
    expect(calls[seeded.id]).toBeUndefined();

    const result = await executor.execute(singleNodeGraph(seeded.id), { seed: 2 });
    expect(result).toEqual(Int32Array.of(2));
    expect(calls[seeded.id]).toBe(1);
  });
});

describe('seed setting', () => {
  it('passes the seed from the settings to seeded nodes', async () => {
    const result = await createExecutor().execute(singleNodeGraph(seeded.id), { seed: 7 });
    expect(result).toEqual(Int32Array.of(7));
  });

  it('defaults to 0 for graphs without a seed', async () => {
    const result = await createExecutor().execute(singleNodeGraph(seeded.id), {});
    expect(result).toEqual(Int32Array.of(0));
  });

  it('gives the same result in every executor with the same seed', async () => {
    const first = await createExecutor().execute(singleNodeGraph(seeded.id), { seed: 42 });
    const second = await createExecutor().execute(singleNodeGraph(seeded.id), { seed: 42 });
    expect(second).toEqual(first);
  });
});

describe('creator inputs', () => {
  beforeEach(() => {
    calls = {};
  });

  it('receive the id of the node they belong to', async () => {
    const executor = createExecutor();
    const result = await executor.execute(singleNodeGraph(creator.id), {});

    // the node under test has id 1
    expect(result).toEqual(Int32Array.of(1));
  });

  it('are the same in every run, so ids made from them stay', async () => {
    const executor = createExecutor();
    const first = await executor.execute(singleNodeGraph(creator.id), { seed: 1 });
    const second = await executor.execute(singleNodeGraph(creator.id), { seed: 1 });

    expect(second).toEqual(first);
  });
});

describe('toCreatorId', () => {
  it('keeps ids that fit into 32 bits', () => {
    expect(toCreatorId(0)).toBe(0);
    expect(toCreatorId(42)).toBe(42);
    expect(toCreatorId(2 ** 31 - 1)).toBe(2 ** 31 - 1);
  });

  it('folds the ids of expanded group nodes into an i32 without losing the difference', () => {
    const a = 2 ** 40 + 5;
    const b = 2 ** 41 + 5;

    for (const id of [a, b, Number.MAX_SAFE_INTEGER]) {
      const folded = toCreatorId(id);
      expect(Number.isInteger(folded)).toBe(true);
      expect(folded).toBeGreaterThanOrEqual(-(2 ** 31));
      expect(folded).toBeLessThan(2 ** 31);
    }
    expect(toCreatorId(a)).not.toBe(toCreatorId(b));
  });
});

describe('runtime reset', () => {
  it('resets modules of node types that left the graph', async () => {
    const executor = createExecutor();
    await executor.execute(singleNodeGraph(seeded.id), {});
    vi.mocked(seeded.reset!).mockClear();

    await executor.execute(singleNodeGraph(setting.id), { resolution: 1 });

    expect(seeded.reset).toHaveBeenCalledTimes(1);
  });
});

describe('runtime node timings', () => {
  it('times every executed node and keeps the time of cached nodes', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    const first = executor.getNodeTimings();

    expect(Object.keys(first).map(Number).sort()).toEqual([0, 1, 2, 3]);

    await executor.execute(graph({ value: 5 }), {});
    const second = executor.getNodeTimings();

    expect(second[3]).toBe(first[3]);
    expect(Object.keys(second).map(Number).sort()).toEqual([0, 1, 2, 3]);
  });

  it('drops nodes that left the graph', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    await executor.execute(singleNodeGraph(left.id), {});

    expect(Object.keys(executor.getNodeTimings()).map(Number).sort()).toEqual([0, 1]);
  });
});

describe('MemoryRuntimeCache', () => {
  const bytes = (n: number) => [new Int32Array(n / 4)];

  it('removes the least recently used values once they hold more than maxBytes', () => {
    const cache = new MemoryRuntimeCache(10, 100);
    cache.set('a', bytes(40));
    cache.set('b', bytes(40));
    cache.get('a');
    cache.set('c', bytes(40));

    expect(cache.get('a')).toBeDefined();
    expect(cache.get('b')).toBeUndefined();
    expect(cache.get('c')).toBeDefined();
    expect(cache.bytes).toBe(80);
  });

  it('counts a replaced value only once', () => {
    const cache = new MemoryRuntimeCache(10, 100);
    cache.set('a', bytes(40));
    cache.set('a', bytes(60));

    expect(cache.bytes).toBe(60);
  });

  it('removes values right away when its limits are lowered', () => {
    const cache = new MemoryRuntimeCache();
    cache.set('a', bytes(40));
    cache.set('b', bytes(40));
    cache.set('c', bytes(40));

    cache.maxBytes = 80;
    expect(cache.get('a')).toBeUndefined();
    expect(cache.bytes).toBe(80);

    cache.size = 1;
    expect(cache.get('b')).toBeUndefined();
    expect(cache.get('c')).toBeDefined();
  });

  it('forgets the bytes of cleared values', () => {
    const cache = new MemoryRuntimeCache();
    cache.set('a', bytes(40));
    cache.clear();

    expect(cache.bytes).toBe(0);
  });
});

describe('runtime cache size', () => {
  beforeEach(() => {
    calls = {};
  });

  it('keeps the results of new seeds to twice those of one run', async () => {
    const cache = new MemoryRuntimeCache();
    const executor = new MemoryRuntimeExecutor(registry, cache);

    for (let seed = 0; seed < 5; seed++) {
      await executor.execute(singleNodeGraph(bigSeeded.id), { seed });
      expect(cache.bytes).toBeLessThanOrEqual(2 * BIG_BYTES);
    }
    expect(cache.maxBytes).toBe(2 * BIG_BYTES);

    // the seed before the last one is still cached, so going back runs nothing
    calls = {};
    await executor.execute(singleNodeGraph(bigSeeded.id), { seed: 3 });
    expect(calls[bigSeeded.id]).toBeUndefined();
  });

  it('keeps every result of a bigger graph than the last one', async () => {
    const cache = new MemoryRuntimeCache();
    const executor = new MemoryRuntimeExecutor(registry, cache);
    await executor.execute(singleNodeGraph(seeded.id), {});

    // output <- add <- (big, big), both big results are over the limit of the last run
    const bigGraph = {
      id: 0,
      nodes: [
        { id: 0, type: output.id, position: [0, 0] },
        { id: 1, type: add.id, position: [0, 0] },
        { id: 2, type: bigSeeded.id, position: [0, 0] },
        { id: 3, type: bigSeeded.id, position: [0, 0] }
      ],
      edges: [
        [1, 0, 0, 'input'],
        [2, 0, 1, 'a'],
        [3, 0, 1, 'b']
      ]
    } as Graph;
    await executor.execute(bigGraph, {});
    calls = {};
    await executor.execute(bigGraph, {});

    expect(calls).toEqual({ [output.id]: 1 });
  });
});
