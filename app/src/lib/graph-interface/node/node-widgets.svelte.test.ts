import { CURRENT_GRAPH_VERSION, debugNode } from '@nodarium/runtime';
import { type Graph, GROUP_INSTANCE, type NodeDefinition } from '@nodarium/types';
import { flushSync } from 'svelte';
import { assert, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { GraphManager } from '../graph-manager.svelte';
import { createMockNodeRegistry } from '../test-utils';
import TestNodes from './TestNodes.svelte';

function definition(id: string, inputs: NodeDefinition['inputs']): NodeDefinition {
  return {
    id,
    inputs,
    outputs: ['path'],
    meta: {},
    execute: () => new Int32Array()
  } as NodeDefinition;
}

const stem = definition('test/widgets/stem', {
  length: { type: 'float', value: 2, min: 0, max: 10 },
  amount: { type: 'integer' }
});
const noise = definition('test/widgets/noise', {
  plant: { type: 'path' },
  strength: { type: 'float', value: 0.3, min: 0, max: 1 },
  fix_bottom: { type: 'boolean', value: false },
  mode: { type: 'select', options: ['soft', 'hard'], value: 1 },
  direction: { type: 'vec3', value: [1, 1, 1] },
  color: { type: 'color', value: [92, 128, 58] },
  curve: { type: 'curve', value: [0, 0, 1, 1] }
});

const graph: Graph = {
  id: 1,
  version: CURRENT_GRAPH_VERSION,
  nodes: [
    { id: 0, type: stem.id, position: [0, 0] },
    { id: 1, type: noise.id, position: [30, 0], props: { fix_bottom: 1 } }
  ],
  edges: [[0, 0, 1, 'plant']],
  groups: []
};

async function setup() {
  const registry = createMockNodeRegistry([stem, noise, debugNode]);
  registry.loadCollection = async () => [];
  const manager = new GraphManager(registry);
  await manager.load(structuredClone(graph));
  const view = await render(TestNodes, { manager });
  const rest = () => {
    flushSync();
    manager.flush();
    flushSync();
  };
  rest();
  const steps = () => manager.history.history.length;
  const input = (nodeId: number, key: string) => {
    const element = view.container.querySelector<HTMLInputElement>(
      `[data-node-id="${nodeId}"] [data-node-input="${key}"] input`
    );
    assert.isNotNull(element);
    return element;
  };
  const type = (nodeId: number, key: string, value: string) => {
    const element = input(nodeId, key);
    element.value = value;
    element.dispatchEvent(new Event('change', { bubbles: true }));
    rest();
  };
  return { manager, rest, steps, input, type };
}

const debugNodes = (manager: GraphManager) =>
  manager.nodeArray.filter((n) => n.type === debugNode.id);
const groupInstances = (manager: GraphManager) =>
  manager.nodeArray.filter((n) => n.type === GROUP_INSTANCE);

describe('the widgets of a node', () => {
  it('write nothing when the nodes of a loaded graph appear', async () => {
    const { manager, steps } = await setup();

    expect(steps()).toBe(0);
    expect(manager.serialize().nodes.map((n) => n.props)).toEqual([undefined, { fix_bottom: 1 }]);
  });

  it('show the default of an input the node has no value for', async () => {
    const { input } = await setup();

    expect(input(0, 'length').value).toBe('2');
    expect(input(1, 'strength').value).toBe('0.3');
  });

  it('write a value the user changes, as one undo step', async () => {
    const { manager, steps, type, input, rest } = await setup();

    type(0, 'length', '5');
    expect(manager.serialize().nodes[0].props).toEqual({ length: 5 });
    expect(steps()).toBe(1);

    manager.undo();
    rest();
    expect(manager.serialize().nodes[0].props).toBeUndefined();
    expect(input(0, 'length').value).toBe('2');
    expect(steps()).toBe(1);

    manager.redo();
    rest();
    expect(manager.serialize().nodes[0].props).toEqual({ length: 5 });
    expect(input(0, 'length').value).toBe('5');
  });

  it('write a changed part of a vector, the default of the definition stays', async () => {
    const { manager, type, steps } = await setup();

    type(1, 'direction', '3');

    expect(manager.serialize().nodes[1].props).toEqual({ fix_bottom: 1, direction: [3, 1, 1] });
    expect(noise.inputs?.direction.value).toEqual([1, 1, 1]);
    expect(steps()).toBe(1);
  });

  it('follow a value changed from outside, like a drag in the viewer', async () => {
    const { manager, input, rest, steps } = await setup();
    const node = manager.getNode(0);
    assert.isDefined(node);

    node.props = { ...node.props, length: 7 };
    manager.commit();
    rest();

    expect(input(0, 'length').value).toBe('7');
    expect(steps()).toBe(1);
  });
});

describe('a new debug node', () => {
  it('is one undo step, and one undo removes it', async () => {
    const { manager, rest, steps } = await setup();

    manager.tryConnectToDebugNode(0);
    rest();
    expect(debugNodes(manager)).toHaveLength(1);
    expect(debugNodes(manager)[0].props).toEqual({});
    expect(steps()).toBe(1);

    manager.undo();
    rest();
    expect(debugNodes(manager)).toHaveLength(0);

    manager.redo();
    rest();
    expect(debugNodes(manager)).toHaveLength(1);
    expect(steps()).toBe(1);
  });
});

describe('grouped nodes', () => {
  it('are one undo step, and one undo gets back past the grouping', async () => {
    const { manager, rest, steps } = await setup();

    manager.groupNodes([1]);
    rest();
    expect(groupInstances(manager)).toHaveLength(1);
    expect(groupInstances(manager)[0].props).toEqual({ groupId: manager.groups[0].id });
    expect(steps()).toBe(1);

    manager.undo();
    rest();
    expect(manager.groups).toEqual([]);
    expect(manager.nodeArray.map((n) => n.type)).toEqual([stem.id, noise.id]);

    manager.redo();
    rest();
    expect(groupInstances(manager)).toHaveLength(1);
    expect(steps()).toBe(1);
  });
});

describe('a group node from the add menu', () => {
  it('is an instance of the first group, in one undo step', async () => {
    const { manager, rest, steps } = await setup();
    manager.groupNodes([1]);
    rest();

    const instance = manager.createNode({ type: GROUP_INSTANCE, position: [0, 50], props: {} });
    rest();

    expect(instance?.props).toEqual({ groupId: manager.groups[0].id });
    expect(steps()).toBe(2);
  });

  it('is not an instance of the group it is placed in', async () => {
    const { manager, rest } = await setup();
    const groupNode = manager.groupNodes([1]);
    assert.isDefined(groupNode);
    manager.enterGroup(groupNode.id);
    rest();

    const instance = manager.createNode({ type: GROUP_INSTANCE, position: [0, 50], props: {} });

    expect(instance?.props).toEqual({});
  });
});
