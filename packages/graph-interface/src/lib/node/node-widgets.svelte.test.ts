import {
  CURRENT_GRAPH_VERSION,
  debugNode,
  type Graph,
  GROUP_INSTANCE,
  type NodeDefinition
} from '@nodarium/runtime';
import { type ComponentProps, flushSync } from 'svelte';
import { SvelteMap } from 'svelte/reactivity';
import { assert, describe, expect, it } from 'vitest';
import { GraphManager } from '../graph-manager.svelte';
import type { InputContextMenuEvent } from '../options';
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

// The widgets only exist in a browser, `vitest --browser=false` has no document for them
const inBrowser = describe.skipIf(import.meta.env.SSR);

async function setup(props: Omit<ComponentProps<typeof TestNodes>, 'manager'> = {}) {
  const { render } = await import('vitest-browser-svelte');
  const registry = createMockNodeRegistry([stem, noise, debugNode]);
  const manager = new GraphManager(registry);
  await manager.load(structuredClone(graph));
  const view = await render(TestNodes, { manager, ...props });
  const rest = () => {
    flushSync();
    manager.flush();
    flushSync();
  };
  rest();
  const steps = () => manager.history.history.length;
  const row = (nodeId: number, key: string) => {
    const element = view.container.querySelector<HTMLElement>(
      `[data-node-id="${nodeId}"] [data-node-input="${key}"]`
    );
    assert.isNotNull(element);
    return element;
  };
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
  return { manager, rest, steps, row, input, type };
}

const debugNodes = (manager: GraphManager) =>
  manager.nodeArray.filter((n) => n.type === debugNode.id);
const groupInstances = (manager: GraphManager) =>
  manager.nodeArray.filter((n) => n.type === GROUP_INSTANCE);

inBrowser('the widgets of a node', () => {
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

inBrowser('a new debug node', () => {
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

inBrowser('grouped nodes', () => {
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

inBrowser('a group node from the add menu', () => {
  it('is an instance of the first group, in one undo step', async () => {
    const { manager, rest, steps } = await setup();
    manager.groupNodes([1]);
    rest();

    const instance = manager.createNode({ type: GROUP_INSTANCE, position: [0, 50], props: {} });
    rest();

    expect(instance?.props).toEqual({ groupId: manager.groups[0].id });
    expect(steps()).toBe(2);
  });
});

inBrowser('the hooks of the app on an input', () => {
  const badgeOf = (row: HTMLElement) => row.querySelector('[data-input-badge]')?.textContent;

  it('draw the badge the app gives and follow it when it changes', async () => {
    const badges = new SvelteMap([['1.strength', 'midi 12']]);
    const { row, rest } = await setup({ inputBadge: (node, key) => badges.get(`${node}.${key}`) });

    expect(badgeOf(row(1, 'strength'))).toBe('midi 12');
    expect(badgeOf(row(0, 'length'))).toBeUndefined();

    badges.set('0.length', 'kick');
    badges.delete('1.strength');
    rest();
    expect(badgeOf(row(0, 'length'))).toBe('kick');
    expect(badgeOf(row(1, 'strength'))).toBeUndefined();
  });

  const rightClick = (element: HTMLElement, init: MouseEventInit = {}) =>
    element.dispatchEvent(
      new MouseEvent('contextmenu', {
        bubbles: true,
        cancelable: true,
        clientX: 4,
        clientY: 8,
        ...init
      })
    );

  it('hand a right click on an input to the app, which can keep the menu of the editor closed', async () => {
    const events: InputContextMenuEvent[] = [];
    const { row } = await setup({
      oninputcontextmenu: (event) => {
        events.push(event);
        if (event.key === 'strength') event.preventDefault();
      }
    });

    expect(rightClick(row(1, 'strength'))).toBe(false);
    expect(events[0]).toMatchObject({
      nodeId: 1,
      nodeType: noise.id,
      key: 'strength',
      input: { type: 'float', max: 1 },
      x: 4,
      y: 8
    });

    // not taken, it goes on to the editor
    expect(rightClick(row(0, 'length'))).toBe(true);
    expect(events).toHaveLength(2);
  });

  it('leave a shift right click to the debug node', async () => {
    const events: InputContextMenuEvent[] = [];
    const { row } = await setup({ oninputcontextmenu: (event) => events.push(event) });

    rightClick(row(1, 'strength'), { shiftKey: true });
    expect(events).toHaveLength(0);
  });
});
