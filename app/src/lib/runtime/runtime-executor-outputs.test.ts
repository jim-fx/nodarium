import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it, vi } from 'vitest';
import { createMockNodeRegistry } from '../graph-interface/test-utils';
import { MemoryRuntimeExecutor } from './runtime-executor';
import { MemoryRuntimeCache } from './runtime-executor-cache';

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

  it('previews a debugged field with the Myzel output node', async () => {
    const field = {
      id: 'max/myzel/sphere',
      inputs: {},
      outputs: ['field'],
      execute: () => Int32Array.of(10, 0, 1, 2, 3, 4, 5, 6)
    } as unknown as NodeDefinition;
    // stands in for the mesher, it reports what it was called with
    const myzelOutput = {
      id: 'max/myzel/output',
      inputs: { field: { type: 'field' }, resolution: { type: 'integer', value: 64 } },
      outputs: [],
      execute: vi.fn(([f, resolution]: Int32Array[]) => Int32Array.of(f[0], resolution[0]))
    } as unknown as NodeDefinition;
    const registry = createMockNodeRegistry([field, myzelOutput, debug]);
    const runtime = new MemoryRuntimeExecutor(registry);

    const result = await runtime.execute(
      graph(
        [node(0, field.id), node(1, debug.id), node(2, myzelOutput.id)],
        [[0, 0, 1, 'input'], [0, 0, 2, 'field']]
      ),
      {}
    );

    // the result is the one of the graph, the preview runs after it was read
    expect([...result]).toEqual([10, 64]);
    expect(runtime.getDebugData()[0]).toEqual({
      type: 'field',
      output: 0,
      data: Int32Array.of(10, 32)
    });
  });

  it('previews a debugged pattern with the Myzel output node', async () => {
    const pattern = {
      id: 'max/myzel/reaction_diffusion',
      inputs: {},
      outputs: ['pattern'],
      execute: () => Int32Array.of(15, 0, 0)
    } as unknown as NodeDefinition;
    const myzelOutput = {
      id: 'max/myzel/output',
      inputs: {
        field: { type: 'field', accepts: ['pattern'] },
        resolution: { type: 'integer', value: 64 }
      },
      outputs: [],
      execute: vi.fn(([p, resolution]: Int32Array[]) => Int32Array.of(p[0], resolution[0]))
    } as unknown as NodeDefinition;
    const registry = createMockNodeRegistry([pattern, myzelOutput, debug]);
    const runtime = new MemoryRuntimeExecutor(registry);

    await runtime.execute(
      graph(
        [node(0, pattern.id), node(1, debug.id), node(2, myzelOutput.id)],
        [[0, 0, 1, 'input'], [0, 0, 2, 'field']]
      ),
      {}
    );

    expect(runtime.getDebugData()[0]).toEqual({
      type: 'pattern',
      output: 0,
      data: Int32Array.of(15, 32)
    });
  });
});
