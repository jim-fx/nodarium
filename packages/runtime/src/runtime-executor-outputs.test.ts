import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it, vi } from 'vitest';
import { debugNode } from './debug-node';
import { MemoryRuntimeExecutor } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';
import { createMockNodeRegistry } from './test-utils';

type TestNode = Graph['nodes'][0];
type TestEdge = Graph['edges'][0];

const node = (id: number, type: string, extra: Partial<TestNode> = {}): TestNode => ({
  id,
  type: type as TestNode['type'],
  position: [0, 0],
  ...extra
});

// splits its value into the value and the value times ten
const split = {
  id: 'test/node/split',
  inputs: { value: { type: 'integer', value: 3 } },
  outputs: ['integer', { type: 'integer', label: 'times ten' }],
  execute: vi.fn(([v]: Int32Array[]) => [Int32Array.of(v[0]), Int32Array.of(v[0] * 10)])
} as unknown as NodeDefinition;

// a + b * 100
const combine = {
  id: 'test/node/combine',
  inputs: { a: { type: 'integer', value: 0 }, b: { type: 'integer', value: 0 } },
  outputs: ['integer'],
  execute: ([a, b]: Int32Array[]) => Int32Array.of(a[0] + b[0] * 100)
} as unknown as NodeDefinition;

const passthrough = {
  id: 'test/node/passthrough',
  inputs: { value: { type: 'integer', value: 0 } },
  outputs: ['integer'],
  execute: () => Int32Array.of(-1)
} as unknown as NodeDefinition;

const output = {
  id: 'test/node/output',
  inputs: { input: { type: 'integer', value: 0 } },
  outputs: [],
  execute: ([v]: Int32Array[]) => v
} as unknown as NodeDefinition;

const debug = {
  id: '__internal/node/debug',
  inputs: { input: { type: '*' } },
  execute: ([v]: Int32Array[]) => v
} as unknown as NodeDefinition;

function setup(cache = false) {
  const registry = createMockNodeRegistry([split, combine, passthrough, output, debug]);
  return new MemoryRuntimeExecutor(registry, cache ? new MemoryRuntimeCache() : undefined);
}

function graph(nodes: TestNode[], edges: TestEdge[]) {
  return { id: 0, groups: [], nodes, edges } as Graph;
}

describe('nodes that feed several nodes', () => {
  it('run before the deepest node they feed', async () => {
    // 0 feeds 3 directly and 4 through 1 and 2, the short way is walked last
    const result = await setup().execute(
      graph(
        [
          node(0, split.id),
          node(1, combine.id),
          node(2, combine.id),
          node(3, combine.id),
          node(4, combine.id),
          node(5, output.id)
        ],
        [
          [0, 0, 1, 'a'],
          [1, 0, 2, 'a'],
          [0, 0, 3, 'a'],
          [3, 0, 4, 'a'],
          [2, 0, 4, 'b'],
          [4, 0, 5, 'input']
        ]
      ),
      {}
    );
    expect([...result]).toEqual([303]);
  });
});

describe('nodes with several outputs', () => {
  it('passes every output to the inputs connected to it', async () => {
    const result = await setup().execute(
      graph(
        [node(0, split.id), node(1, combine.id), node(2, output.id)],
        [[0, 1, 1, 'a'], [0, 0, 1, 'b'], [1, 0, 2, 'input']]
      ),
      {}
    );
    // a = 30, b = 3
    expect([...result]).toEqual([330]);
  });

  it('caches every output', async () => {
    const runtime = setup(true);
    const g = (value: number) =>
      graph(
        [
          node(0, split.id, { props: { value: 3 } }),
          node(1, combine.id, { props: { b: value } }),
          node(2, output.id)
        ],
        [[0, 1, 1, 'a'], [1, 0, 2, 'input']]
      );

    vi.mocked(split.execute).mockClear();
    expect([...await runtime.execute(g(0), {})]).toEqual([30]);
    expect([...await runtime.execute(g(1), {})]).toEqual([130]);
    expect(split.execute).toHaveBeenCalledTimes(1);
  });

  it('keeps the output a muted node passes through', async () => {
    const result = await setup().execute(
      graph(
        [node(0, split.id), node(1, passthrough.id, { muted: true }), node(2, output.id)],
        [[0, 1, 1, 'value'], [1, 0, 2, 'input']]
      ),
      {}
    );
    expect([...result]).toEqual([30]);
  });

  it('shows the output a debug node is connected to', async () => {
    const runtime = setup();
    await runtime.execute(
      graph(
        [node(0, split.id), node(1, debug.id), node(2, output.id)],
        [[0, 1, 1, 'input'], [0, 0, 2, 'input']]
      ),
      {}
    );
    expect(runtime.getDebugData()[0]).toEqual({
      type: 'integer',
      output: 1,
      data: Int32Array.of(30)
    });
  });

  it('shows the output a debug node is connected to when the nodes below it come from the cache', async () => {
    const runtime = setup(true);
    const g = graph(
      [node(0, split.id), node(1, debug.id), node(2, combine.id), node(3, output.id)],
      [[0, 1, 1, 'input'], [0, 0, 2, 'a'], [2, 0, 3, 'input']]
    );
    await runtime.execute(g, {});
    await runtime.execute(g, {});
    expect(runtime.getDebugData()[0]).toEqual({
      type: 'integer',
      output: 1,
      data: Int32Array.of(30)
    });
  });

  describe('debugging what the Myzel output shows', () => {
    const maker = (id: string, type: string, words: number[]) =>
      ({
        id,
        inputs: {},
        outputs: [type],
        execute: () => Int32Array.of(...words)
      }) as unknown as NodeDefinition;
    const myzelOutput = {
      id: 'max/myzel/output',
      inputs: { mesh: { type: 'path', accepts: ['mesh', 'path', 'pattern'] } },
      outputs: [],
      execute: vi.fn(([p]: Int32Array[]) => Int32Array.of(p[0], 99))
    } as unknown as NodeDefinition;

    async function debugged(source: NodeDefinition) {
      const runtime = new MemoryRuntimeExecutor(
        createMockNodeRegistry([source, myzelOutput, debug])
      );
      await runtime.execute(
        graph(
          [node(0, source.id), node(1, debug.id), node(2, myzelOutput.id)],
          [[0, 0, 1, 'input'], [0, 0, 2, 'mesh']]
        ),
        {}
      );
      return runtime.getDebugData()[0];
    }

    it('previews a debugged pattern with the output node', async () => {
      const entry = await debugged(maker('max/myzel/reaction_diffusion', 'pattern', [15, 0, 0]));
      expect(entry).toEqual({ type: 'pattern', output: 0, data: Int32Array.of(15, 99) });
    });

    it('draws a debugged mesh as it is', async () => {
      const entry = await debugged(maker('max/myzel/join_meshes', 'mesh', [14, 1, 2]));
      expect(entry).toEqual({ type: 'mesh', output: 0, data: Int32Array.of(14, 1, 2) });
    });

    it('draws debugged paths as they are', async () => {
      const entry = await debugged(maker('max/myzel/strands', 'path', [0, 5, 1, 1]));
      expect(entry).toEqual({ type: 'path', output: 0, data: Int32Array.of(0, 5, 1, 1) });
    });
  });

  describe('the amount and clip of the debug node', () => {
    function run(runtime: MemoryRuntimeExecutor, props?: Record<string, number>) {
      return runtime.execute(
        graph(
          [node(0, split.id), node(1, debugNode.id, { props }), node(2, output.id)],
          [[0, 1, 1, 'input'], [0, 0, 2, 'input']]
        ),
        {}
      );
    }
    const registry = () => createMockNodeRegistry([split, output, debugNode]);

    it('travel to the viewer with the debug data, which stays as it is', async () => {
      const runtime = new MemoryRuntimeExecutor(registry());
      await run(runtime, { amount: 0.2, clip: 0.5 });
      const entry = runtime.getDebugData()[0];
      expect(entry.data).toEqual(Int32Array.of(30));
      expect(entry.amount).toBeCloseTo(0.2);
      expect(entry.clip).toBe(0.5);
    });

    it('are 1 by default', async () => {
      const runtime = new MemoryRuntimeExecutor(registry());
      await run(runtime);
      expect(runtime.getDebugData()[0]).toEqual({
        type: 'integer',
        output: 1,
        data: Int32Array.of(30),
        amount: 1,
        clip: 1
      });
    });

    it('do not run the debugged node again or change the result', async () => {
      const runtime = new MemoryRuntimeExecutor(registry(), new MemoryRuntimeCache());
      vi.mocked(split.execute).mockClear();
      const before = await run(runtime);
      const after = await run(runtime, { amount: 0.05, clip: 0.1 });
      expect(split.execute).toHaveBeenCalledTimes(1);
      expect([...after]).toEqual([...before]);
      expect(runtime.getDebugData()[0].amount).toBeCloseTo(0.05);
    });
  });
});
