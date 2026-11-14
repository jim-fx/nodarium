import { CURRENT_GRAPH_VERSION } from '@nodarium/runtime';
import type { Graph, NodeDefinition, NodeInstance, SerializedNode } from '@nodarium/types';
import { GROUP_INSTANCE } from '@nodarium/types';
import { afterEach, assert, beforeEach, describe, expect, it, vi } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { serializeEdge } from './helpers/nodeHelpers';
import { createMockNodeRegistry, mockFloatInputNode, mockFloatOutputNode } from './test-utils';

const templates = {
  ...import.meta.glob<Graph>('../graph-templates/*.json', { eager: true, import: 'default' }),
  ...import.meta.glob<Graph>('../../../../packages/runtime/templates/*.json', {
    eager: true,
    import: 'default'
  })
};
const definitions = Object.values(
  import.meta.glob<NodeDefinition>('../../../../nodes/*/*/*/src/definition.json', {
    eager: true,
    import: 'default'
  })
);

// serialize() as it was before it walked the fields itself
function legacySerialize(manager: GraphManager): Graph {
  const serializeNode = (node: SerializedNode | NodeInstance) => {
    const muted = 'state' in node ? node.state.muted : node.muted;
    return {
      id: node.id,
      position: [...node.position],
      type: node.type,
      props: node.props ? JSON.parse(JSON.stringify(node.props)) : undefined,
      ...(node.exposed?.length ? { exposed: [...node.exposed] } : {}),
      ...(muted ? { muted: true } : {})
    };
  };
  const levelState = (id: number | null) => manager['levelState'](id);
  const root = levelState(null);
  const groups = manager.groups?.map((group) => {
    const level = levelState(group.id);
    return {
      id: group.id,
      name: group.name,
      inputs: group.inputs,
      outputs: group.outputs,
      nodes: level.nodes.map(serializeNode),
      edges: level.edges.map(serializeEdge)
    };
  });
  const serialized = $state.snapshot({
    id: manager.id,
    version: CURRENT_GRAPH_VERSION,
    settings: manager.settings,
    meta: manager.meta,
    groups,
    nodes: root.nodes.map(serializeNode),
    edges: root.edges.map(serializeEdge)
  });
  return JSON.parse(JSON.stringify(serialized));
}

async function loadTemplate(graph: Graph) {
  const registry = createMockNodeRegistry(definitions);
  registry.loadDefinitions = async () => [];
  registry.loadCollection = async () => [];
  const manager = new GraphManager(registry);
  await manager.load(structuredClone(graph));
  return manager;
}

describe('serialize', () => {
  it.each(Object.entries(templates))('gives the same graph as before for %s', async (_, graph) => {
    const manager = await loadTemplate(graph);
    expect(manager.status).toBe('idle');
    manager.muteNode(manager.nodeArray[0].id);

    expect(manager.serialize()).toStrictEqual(legacySerialize(manager));

    const instance = manager.nodeArray.find((n) => n.type === GROUP_INSTANCE);
    if (instance && manager.enterGroup(instance.id)) {
      expect(manager.serialize()).toStrictEqual(legacySerialize(manager));
    }
  });

  it('writes non-finite numbers and holes the way JSON does', () => {
    const manager = new GraphManager(createMockNodeRegistry([mockFloatInputNode]));
    const node = manager.createNode({
      type: 'test/node/input',
      position: [0, -0],
      props: {
        value: NaN,
        list: [1, undefined, Infinity],
        nested: { gone: undefined, kept: -0 }
      } as unknown as NodeInstance['props']
    });
    assert.isDefined(node);

    expect(manager.serialize()).toStrictEqual(legacySerialize(manager));
  });

  it('hands out copies', () => {
    const manager = new GraphManager(createMockNodeRegistry([mockFloatInputNode]));
    manager.createNode({ type: 'test/node/input', position: [0, 0], props: { value: 1 } });

    const first = manager.serialize();
    first.nodes[0].props!.value = 2;
    expect(manager.serialize().nodes[0].props!.value).toBe(1);
  });

  // The server build of Svelte, which `vitest --browser=false` compiles this file with, has no
  // reactivity, so its $derived can't know that nothing changed and runs on every read
  it.skipIf(import.meta.env.SSR)(
    'walks the graph once for the undo step and the build of one change',
    () => {
      vi.useFakeTimers();
      const manager = new GraphManager(createMockNodeRegistry([mockFloatInputNode]));
      const node = manager.createNode({
        type: 'test/node/input',
        position: [0, 0],
        props: { value: 1 }
      });
      assert.isDefined(node);
      manager.loaded = true;
      vi.runAllTimers();
      const walk = vi.spyOn(
        manager as unknown as { serializeState: () => Graph },
        'serializeState'
      );
      const results: Graph[] = [];
      manager.on('result', (g) => results.push(g as Graph));

      node.props = { value: 2 };
      manager.save();
      manager.execute();

      expect(walk).toHaveBeenCalledTimes(1);
      expect(results.at(-1)?.nodes[0].props).toEqual({ value: 2 });
      vi.useRealTimers();
    }
  );
});

describe('a value dragged in an input', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  function setup() {
    const manager = new GraphManager(
      createMockNodeRegistry([mockFloatInputNode, mockFloatOutputNode])
    );
    const node = manager.createNode({
      type: 'test/node/input',
      position: [0, 0],
      props: { value: 1 }
    });
    assert.isDefined(node);
    manager.save();
    vi.runAllTimers();
    return { manager, node };
  }

  function drag(manager: GraphManager, node: NodeInstance, values: number[]) {
    for (const value of values) {
      node.props = { ...node.props, value };
      manager.save({ coalesce: true });
      manager.execute();
      vi.advanceTimersByTime(16);
    }
  }

  const valueOf = (manager: GraphManager) => manager.serialize().nodes[0].props?.value;

  it('becomes one undo step that holds the value from before the drag', () => {
    const { manager, node } = setup();
    const steps = manager.history.history.length;

    drag(manager, node, Array.from({ length: 30 }, (_, i) => 2 + i));
    vi.runAllTimers();

    expect(manager.history.history.length).toBe(steps + 1);
    manager.undo();
    expect(valueOf(manager)).toBe(1);
    manager.redo();
    expect(valueOf(manager)).toBe(31);
  });

  it('can be undone before it rests', () => {
    const { manager, node } = setup();
    const steps = manager.history.history.length;

    drag(manager, node, [2, 3, 4]);
    manager.undo();

    expect(manager.history.history.length).toBe(steps + 1);
    expect(valueOf(manager)).toBe(1);
  });

  it('builds every value', () => {
    const { manager, node } = setup();
    manager.loaded = true;
    const results: Graph[] = [];
    manager.on('result', (g) => results.push(g as Graph));

    drag(manager, node, [2, 3, 4]);

    expect(results.map((g) => g.nodes[0].props?.value)).toEqual([2, 3, 4]);
  });

  it('is saved once it rests, or right away on flush()', () => {
    const { manager, node } = setup();
    const saves: Graph[] = [];
    manager.on('save', (g) => saves.push(g));

    drag(manager, node, [2, 3]);
    expect(saves).toHaveLength(0);
    vi.runAllTimers();
    expect(saves.map((g) => g.nodes[0].props?.value)).toEqual([3]);

    drag(manager, node, [4]);
    manager.flush();
    expect(saves.map((g) => g.nodes[0].props?.value)).toEqual([3, 4]);
    vi.runAllTimers();
    expect(saves).toHaveLength(2);
  });
});
