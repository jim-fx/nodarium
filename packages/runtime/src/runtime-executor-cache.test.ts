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

  it('keeps the times of the nodes that ran apart from the cached ones', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    expect(Object.keys(executor.getRunTimings()).map(Number).sort()).toEqual([0, 1, 2, 3]);

    await executor.execute(graph({ value: 5 }), {});
    expect(Object.keys(executor.getRunTimings()).map(Number).sort()).toEqual([0, 1, 2]);
  });

  it('gives a group node the sum of the nodes inside it', async () => {
    const executor = createExecutor();
    await executor.execute(
      {
        id: 0,
        nodes: [
          { id: 0, type: output.id, position: [0, 0] },
          { id: 5, type: '__internal/group/instance', position: [0, 0], props: { groupId: 9 } }
        ],
        edges: [[5, 0, 0, 'input']],
        groups: [{
          id: 9,
          nodes: [
            { id: 1, type: add.id, position: [0, 0] },
            { id: 2, type: left.id, position: [0, 0] },
            { id: 3, type: right.id, position: [0, 0] },
            { id: 4, type: '__internal/group/output', position: [0, 0] }
          ],
          edges: [[2, 0, 1, 'a'], [3, 0, 1, 'b'], [1, 0, 4, 'Out']],
          outputs: [{ type: 'float' }]
        }]
      } as Graph,
      {}
    );
    const timings = executor.getNodeTimings();
    const inside = Object.entries(timings).filter(([id]) => id !== '0' && id !== '5');

    expect(inside).toHaveLength(3);
    expect(timings[5]).toBeCloseTo(inside.reduce((sum, [, time]) => sum + time, 0));
    expect(executor.getRunTimings()[5]).toBeCloseTo(timings[5]);
  });

  it('drops nodes that left the graph', async () => {
    const executor = createExecutor();
    await executor.execute(graph(), {});
    await executor.execute(singleNodeGraph(left.id), {});

    expect(Object.keys(executor.getNodeTimings()).map(Number).sort()).toEqual([0, 1]);
  });
});

const MB = 2 ** 20;

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

  it('keeps small values, and big ones only when they took at least 1 ms per MB', () => {
    const cache = new MemoryRuntimeCache();
    cache.set('small', bytes(MB), 0);
    cache.set('cheap', bytes(64 * MB), 32);
    cache.set('slow', bytes(64 * MB), 128);

    expect(cache.keeps(64 * MB, 32)).toBe(false);
    expect(cache.get('small')).toBeDefined();
    expect(cache.get('cheap')).toBeUndefined();
    expect(cache.get('slow')).toBeDefined();
    expect(cache.bytes).toBe(65 * MB);
  });

  it('keeps the values of the current run over older ones', () => {
    const cache = new MemoryRuntimeCache();
    cache.startRun();
    cache.set('old', bytes(40), 1000);
    cache.startRun();
    cache.set('new', bytes(40), 1);

    cache.maxBytes = 40;
    expect(cache.get('old')).toBeUndefined();
    expect(cache.get('new')).toBeDefined();
  });

  it('counts a value read in this run as part of it', () => {
    const cache = new MemoryRuntimeCache();
    cache.startRun();
    cache.set('read', bytes(40), 1);
    cache.set('unread', bytes(40), 1000);
    cache.startRun();
    cache.get('read');

    cache.maxBytes = 40;
    expect(cache.get('unread')).toBeUndefined();
    expect(cache.get('read')).toBeDefined();
  });

  it('removes the older value that saves the least time per byte first', () => {
    const cache = new MemoryRuntimeCache();
    cache.startRun();
    cache.set('big', bytes(80), 10);
    cache.set('small', bytes(40), 10);
    cache.startRun();

    cache.maxBytes = 80;
    expect(cache.get('big')).toBeUndefined();
    expect(cache.get('small')).toBeDefined();
  });

  it('lets a value that saves much go once it was not used for a few runs', () => {
    const cache = new MemoryRuntimeCache();
    cache.startRun();
    cache.set('slow', bytes(40), 10);
    cache.startRun();
    cache.set('fast', bytes(40), 3);
    cache.startRun();

    cache.maxBytes = 40;
    // `slow` saves 10 for 2 runs, `fast` 3 for 1
    expect(cache.get('fast')).toBeUndefined();

    cache.clear();
    cache.maxBytes = Infinity;
    cache.startRun();
    cache.set('slow', bytes(40), 10);
    cache.startRun();
    cache.set('fast', bytes(40), 6);
    cache.startRun();

    cache.maxBytes = 40;
    expect(cache.get('slow')).toBeUndefined();
  });

  it('removes the least recently used of older runs when it holds too many values', () => {
    const cache = new MemoryRuntimeCache(2);
    cache.startRun();
    cache.set('a', bytes(4), 1);
    cache.set('b', bytes(4), 1);
    cache.startRun();
    cache.get('a');
    cache.set('c', bytes(4), 1);

    expect(cache.get('b')).toBeUndefined();
    expect(cache.get('a')).toBeDefined();
    expect(cache.get('c')).toBeDefined();
  });
});

describe('runtime cache by cost', () => {
  const MB_INTS = 2 ** 18;
  // a mesh that is passed on: 4 MB, made at once
  const bigCheap = {
    id: 'test/cache/big-cheap',
    inputs: { value: { type: 'integer', value: 1 } },
    outputs: ['float'],
    execute: vi.fn((values: NodeValue[]) => {
      calls['test/cache/big-cheap'] = (calls['test/cache/big-cheap'] ?? 0) + 1;
      return new Int32Array(4 * MB_INTS).fill((values[0] as Int32Array)[0]);
    })
  } as NodeDefinition;
  const first = sumNode('test/cache/first', { input: { type: 'float' } });
  const costRegistry = createMockNodeRegistry([output, add, right, first, bigCheap]);

  beforeEach(() => {
    calls = {};
  });

  function costGraph(rightProps?: Record<string, number>): Graph {
    // output <- add <- (first <- big cheap, right)
    return {
      id: 0,
      nodes: [
        { id: 0, type: output.id, position: [0, 0] },
        { id: 1, type: add.id, position: [0, 0] },
        { id: 2, type: first.id, position: [0, 0] },
        { id: 3, type: bigCheap.id, position: [0, 0] },
        { id: 4, type: right.id, position: [0, 0], ...(rightProps ? { props: rightProps } : {}) }
      ],
      edges: [
        [1, 0, 0, 'input'],
        [2, 0, 1, 'a'],
        [3, 0, 2, 'input'],
        [4, 0, 1, 'b']
      ]
    } as Graph;
  }

  it('does not store a big result that is quick to make', async () => {
    const cache = new MemoryRuntimeCache();
    const executor = new MemoryRuntimeExecutor(
      createMockNodeRegistry([output, bigCheap]),
      cache
    );
    await executor.execute(singleNodeGraph(bigCheap.id), {});
    calls = {};
    await executor.execute(singleNodeGraph(bigCheap.id), {});

    expect(calls).toEqual({ [bigCheap.id]: 1, [output.id]: 1 });
    expect(cache.bytes).toBe(0);
  });

  it('does not run a node when the cache has the results of the nodes it feeds', async () => {
    const executor = new MemoryRuntimeExecutor(costRegistry, new MemoryRuntimeCache());
    await executor.execute(costGraph(), {});
    calls = {};

    const result = await executor.execute(costGraph({ value: 5 }), {});

    expect(result).toEqual(Int32Array.of(6));
    expect(calls).toEqual({ [right.id]: 1, [add.id]: 1, [output.id]: 1 });
    expect(executor.getRunTimings()).not.toHaveProperty('3');
    expect(executor.getNodeTimings()).toHaveProperty('3');
  });

  it('counts the warm time of a node that had to be instantiated again', async () => {
    vi.useFakeTimers({ toFake: ['performance'] });
    try {
      let memory = 0;
      const coldOrWarm = {
        ...bigCheap,
        id: 'test/cache/cold-or-warm',
        execute: vi.fn((values: NodeValue[]) => {
          calls['test/cache/cold-or-warm'] = (calls['test/cache/cold-or-warm'] ?? 0) + 1;
          // 4 MB in 1 ms warm, in 40 ms with the instance to set up first
          vi.advanceTimersByTime(memory ? 1 : 40);
          memory = 1;
          return new Int32Array(4 * MB_INTS).fill((values[0] as Int32Array)[0]);
        }),
        memoryBytes: () => memory,
        release: () => {
          memory = 0;
        }
      } as NodeDefinition;
      const executor = new MemoryRuntimeExecutor(
        createMockNodeRegistry([output, coldOrWarm]),
        new MemoryRuntimeCache()
      );
      const run = (value: number) =>
        executor.execute(
          {
            ...singleNodeGraph(coldOrWarm.id),
            nodes: [
              { id: 0, type: output.id, position: [0, 0] },
              { id: 1, type: coldOrWarm.id, position: [0, 0], props: { value } }
            ]
          } as Graph,
          {}
        );

      await run(1);
      await run(2);
      executor.releaseInstances();
      await run(3);
      calls = {};
      await run(3);

      expect(calls[coldOrWarm.id]).toBe(1);
    } finally {
      vi.useRealTimers();
    }
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
