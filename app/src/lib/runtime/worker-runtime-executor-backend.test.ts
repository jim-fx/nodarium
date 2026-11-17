import { CURRENT_GRAPH_VERSION, NodeExecutionError } from '@nodarium/runtime';
import type { Graph, NodeDefinition, NodeValue } from '@nodarium/types';
import { describe, expect, it, vi } from 'vitest';
import { errorInfo, unwrapExecution, WorkerRunError } from './run-error';

const nodes = vi.hoisted(() => {
  const output = {
    id: 'test/node/output',
    inputs: { input: { type: 'path', accepts: ['geometry'], external: true } },
    outputs: [],
    execute: (inputs: NodeValue[]) => inputs[0]
  } as unknown as NodeDefinition;
  const stem = {
    id: 'test/node/stem',
    inputs: {},
    outputs: ['path'],
    execute: () => Int32Array.of(0, 1, 1, 1)
  } as unknown as NodeDefinition;
  const leaf = {
    id: 'test/node/leaf',
    meta: { title: 'Leaf' },
    inputs: { shape: { type: 'shape', external: true }, size: { type: 'float', value: 1 } },
    outputs: ['geometry'],
    execute: () => {
      throw new Error('index out of bounds');
    }
  } as unknown as NodeDefinition;
  const twig = { ...stem, id: 'test/node/twig' } as NodeDefinition;
  return { output, stem, leaf, twig };
});

// the registry of the worker fetches its nodes and keeps them in IndexedDB, a download takes 500ms
vi.mock('#lib/node-registry/index.ts', async () => {
  const { debugNode } = await import('@nodarium/runtime');
  return {
    createAppRegistry: (builtin: NodeDefinition[] = [debugNode]) => {
      const all = new Map<string, NodeDefinition>(
        [...builtin, ...Object.values(nodes)].map((n) => [n.id, n])
      );
      const loaded = new Map<string, NodeDefinition>(builtin.map((n) => [n.id, n]));
      return {
        status: 'loading',
        async load(ids: string[]) {
          for (const id of ids) {
            const node = all.get(id);
            if (!node || loaded.has(id)) continue;
            if (vi.isFakeTimers()) vi.advanceTimersByTime(500);
            loaded.set(id, node);
          }
          return ids.flatMap((id) => loaded.get(id) ?? []);
        },
        getNode: (id: string) => loaded.get(id)
      };
    }
  };
});

const { executeGraph } = await import('./worker-runtime-executor-backend');

const node = (id: number, type: string, props?: Record<string, number>) =>
  ({ id, type, position: [0, 0], ...(props ? { props } : {}) }) as Graph['nodes'][0];

/** The output and a debug node show node 1 */
function graphOf(source: string, groups: Graph['groups'] = []): Graph {
  return {
    id: 0,
    version: CURRENT_GRAPH_VERSION,
    nodes: [
      node(0, nodes.output.id),
      node(1, source, groups.length ? { groupId: groups[0].id } : undefined),
      node(2, '__internal/node/debug')
    ],
    edges: [[1, 0, 0, 'input'], [1, 0, 2, 'input']],
    groups
  } as Graph;
}

/** The answer as the page receives it, a message only carries what can be cloned */
async function answerOf(graph: Graph) {
  return structuredClone(await executeGraph(graph, {}));
}

describe('the answer of the runtime worker', () => {
  it('carries the result, the debug data, the timings and the performance of a run', async () => {
    const answer = await answerOf(graphOf(nodes.stem.id));

    expect(answer).toMatchObject({
      ok: true,
      result: Int32Array.of(0, 1, 1, 1),
      debugData: { 1: { type: 'path', output: 0, data: Int32Array.of(0, 1, 1, 1) } }
    });
    const execution = unwrapExecution(answer);
    expect(Object.keys(execution.nodeTimings).map(Number).sort()).toEqual([0, 1, 2]);
    expect(Object.keys(execution.runTimings).map(Number).sort()).toEqual([0, 1, 2]);
    expect(execution.performance?.total).toHaveLength(1);
    expect(execution.performance?.['node/' + nodes.stem.id]).toHaveLength(1);
  });

  it('names the failed node and its unconnected inputs', async () => {
    const answer = await answerOf(graphOf(nodes.leaf.id));

    expect(answer).toEqual({
      ok: false,
      error: {
        message: 'Leaf failed: index out of bounds (unconnected input "shape")',
        nodeId: 1,
        nodeType: nodes.leaf.id,
        nodeTitle: 'Leaf',
        missingInputs: ['shape']
      }
    });
  });

  it('points an error inside a group at the group node', async () => {
    const answer = await answerOf(
      graphOf(
        '__internal/group/instance',
        [{
          id: 1,
          inputs: {},
          nodes: [node(10, '__internal/group/output'), node(11, nodes.leaf.id)],
          edges: [[11, 0, 10, 'out_0']]
        }] as unknown as Graph['groups']
      )
    );

    expect(answer).toMatchObject({ ok: false, error: { nodeId: 1, nodeType: nodes.leaf.id } });
  });

  it('reports only its own time and points after a failed run', async () => {
    vi.useFakeTimers({ toFake: ['performance'] });
    try {
      await answerOf(graphOf(nodes.leaf.id));
      vi.advanceTimersByTime(300);
      const execution = unwrapExecution(await answerOf(graphOf(nodes.stem.id)));

      expect(execution.performance?.total).toEqual([0]);
      expect(execution.performance?.['collect-metadata']).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not count the downloads of the nodes in the time of the first run', async () => {
    vi.useFakeTimers({ toFake: ['performance'] });
    try {
      const execution = unwrapExecution(await answerOf(graphOf(nodes.twig.id)));

      expect(execution.performance?.total).toEqual([0]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('answers a graph that can not run with the reason', async () => {
    const answer = await answerOf({
      id: 0,
      nodes: [node(1, nodes.stem.id)],
      edges: [],
      groups: []
    });

    expect(answer).toEqual({ ok: false, error: { message: 'No output node found' } });
  });
});

describe('a failed run', () => {
  it('of the worker throws with the error of its answer', async () => {
    const answer = await answerOf(graphOf(nodes.leaf.id));

    const error = (() => {
      try {
        return unwrapExecution(answer);
      } catch (e) {
        return e;
      }
    })();
    expect(error).toBeInstanceOf(WorkerRunError);
    expect((error as Error).message).toBe(
      'Leaf failed: index out of bounds (unconnected input "shape")'
    );
    expect(errorInfo(error)).toEqual(answer.ok ? undefined : answer.error);
  });

  it('on the main thread is described by its error', () => {
    const error = new NodeExecutionError('broken', { id: 4, type: nodes.leaf.id }, nodes.leaf);

    expect(errorInfo(error)).toMatchObject({ message: 'Leaf failed: broken', nodeId: 4 });
    expect(errorInfo(new Error('no registry'))).toEqual({ message: 'no registry' });
    expect(errorInfo('cancelled')).toEqual({ message: 'cancelled' });
  });
});
