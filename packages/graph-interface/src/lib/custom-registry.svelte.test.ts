import {
  CORE_COLLECTION,
  CURRENT_GRAPH_VERSION,
  type Graph,
  type NodeDefinition,
  type NodeId,
  type NodeRegistry
} from '@nodarium/runtime';
import { flushSync } from 'svelte';
import { afterEach, assert, describe, expect, it, vi } from 'vitest';
import GraphInterface from './graph/Wrapper.svelte';
import { createGraphOptions } from './test-utils';

const wasmDefinitions = Object.values(
  import.meta.glob<NodeDefinition>('../../../../nodes/max/*/*/src/definition.json', {
    eager: true,
    import: 'default'
  })
).map((definition) => ({ ...definition, execute: () => new Int32Array() }));

// JS nodes of an app like nodarium-vj: its own collection and an output that takes paths
const circle: NodeDefinition = {
  id: 'max/vj/circle',
  inputs: { radius: { type: 'float', value: 1, min: 0, max: 4 } },
  outputs: ['path'],
  meta: { title: 'Circle' },
  execute: () => new Int32Array()
};
const output: NodeDefinition = {
  id: 'max/vj/output',
  inputs: { paths: { type: 'path' } },
  outputs: [],
  meta: { title: 'Output' },
  execute: () => new Int32Array()
};

/** The registry an app writes itself: its JS nodes next to the wasm nodes, nothing optional */
class AppRegistry implements NodeRegistry {
  status = 'ready' as const;
  private nodes = new Map<string, NodeDefinition>(
    [...wasmDefinitions, circle, output].map((n) => [n.id, n])
  );
  private loaded = new Map<string, NodeDefinition>();
  baseCollections: `${string}/${string}`[] = [CORE_COLLECTION, 'max/vj'];

  async load(ids: NodeId[]) {
    return ids.map((id) => {
      const node = this.nodes.get(id);
      if (!node) throw new Error(`unknown node ${id}`);
      this.loaded.set(id, node);
      return node;
    });
  }
  async loadCollection(collection: `${string}/${string}`) {
    const nodes = [...this.nodes.values()].filter((n) => n.id.startsWith(`${collection}/`));
    for (const node of nodes) this.loaded.set(node.id, node);
    return nodes;
  }
  getNode(id: string) {
    return this.loaded.get(id);
  }
  getAllNodes() {
    return [...this.loaded.values()];
  }
  async register(): Promise<NodeDefinition> {
    throw new Error('no dropped nodes');
  }
}

const graph: Graph = {
  id: 7,
  version: CURRENT_GRAPH_VERSION,
  nodes: [
    { id: 0, type: 'max/vj/output', position: [40, 0] },
    { id: 1, type: 'max/plantarium/stem', position: [0, 0] },
    { id: 2, type: 'max/plantarium/noise', position: [20, 0] }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 0, 'paths']],
  groups: []
};

describe.skipIf(import.meta.env.SSR)('the editor with a registry of the app', () => {
  afterEach(() => vi.restoreAllMocks());

  it('loads and edits a graph that ends in an output of its own', async () => {
    const errors = vi.spyOn(console, 'error');
    const { render } = await import('vitest-browser-svelte');
    const results: Graph[] = [];
    const view = await render(GraphInterface, {
      graph: structuredClone(graph),
      registry: new AppRegistry(),
      options: createGraphOptions({ valuesOnly: false }),
      onresult: (result) => results.push(result)
    });
    const manager = view.component.manager;
    await vi.waitFor(() => expect(manager.status).toBe('idle'));
    await vi.waitFor(() => expect(results.length).toBeGreaterThan(0));
    expect(results.at(-1)!.nodes.map((n) => n.type)).toContain('max/vj/output');

    const offered = manager.getNodeDefinitions().map((d) => d.id);
    expect(offered).toEqual(expect.arrayContaining(['max/vj/circle', 'max/core/float']));

    const added = manager.createNode({ type: 'max/vj/circle', position: [20, 20], props: {} });
    assert.isDefined(added);
    const target = manager.getNode(0)!;
    expect(manager.createEdge(added, 0, target, 'paths')).toBeDefined();
    flushSync();
    manager.flush();

    const saved = manager.serialize();
    expect(saved.edges).toContainEqual([added.id, 0, 0, 'paths']);
    expect(saved.edges).not.toContainEqual([2, 0, 0, 'paths']);

    manager.undo();
    expect(manager.serialize().edges).toContainEqual([2, 0, 0, 'paths']);
    expect(view.container.querySelector('[data-node-id="0"]')).not.toBeNull();
    expect(errors).not.toHaveBeenCalled();
  });
});
