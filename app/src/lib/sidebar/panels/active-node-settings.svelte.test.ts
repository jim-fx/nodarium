import { GraphManager } from '#lib/graph-interface/graph-manager.svelte.ts';
import { createMockNodeRegistry } from '#lib/graph-interface/test-utils.ts';
import { CURRENT_GRAPH_VERSION } from '@nodarium/runtime';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { flushSync } from 'svelte';
import { assert, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ActiveNodeSettings from './ActiveNodeSettings.svelte';

const random = {
  id: 'test/panel/random',
  inputs: {
    min: { type: 'float', value: 0 },
    depth: { type: 'integer', value: 1, min: 1, max: 10, hidden: true },
    uniform: { type: 'boolean', value: false, hidden: true },
    mode: { type: 'select', options: ['soft', 'hard'], hidden: true }
  },
  outputs: ['float'],
  meta: {},
  execute: () => new Int32Array()
} as unknown as NodeDefinition;

const graph: Graph = {
  id: 1,
  version: CURRENT_GRAPH_VERSION,
  nodes: [{ id: 0, type: random.id, position: [0, 0], props: { min: 0.2 } }],
  edges: [],
  groups: []
};

async function setup() {
  const registry = createMockNodeRegistry([random]);
  registry.loadCollection = async () => [];
  const manager = new GraphManager(registry);
  await manager.load(structuredClone(graph));
  const rest = () => {
    flushSync();
    manager.flush();
    flushSync();
  };
  const show = async () => {
    const node = manager.getNode(0);
    assert.isDefined(node);
    const view = await render(ActiveNodeSettings, { manager, node });
    rest();
    return view;
  };
  const steps = () => manager.history.history.length;
  const props = () => manager.serialize().nodes[0].props;
  return { manager, rest, show, steps, props };
}

describe('the settings of the active node', () => {
  it('write nothing when the node is selected', async () => {
    const { show, steps, props } = await setup();

    await show();

    expect(props()).toEqual({ min: 0.2 });
    expect(steps()).toBe(0);
  });

  it('write a value the user changes, as one undo step', async () => {
    const { show, rest, steps, props } = await setup();
    const view = await show();

    const depth = view.container.querySelector<HTMLInputElement>('input[type="number"]');
    assert.isNotNull(depth);
    depth.value = '4';
    depth.dispatchEvent(new Event('change', { bubbles: true }));
    rest();

    expect(props()).toEqual({ min: 0.2, depth: 4 });
    expect(steps()).toBe(1);
  });

  it('let undo get past the change while the node stays selected', async () => {
    const { manager, show, rest, steps, props } = await setup();
    const view = await show();
    const depth = view.container.querySelector<HTMLInputElement>('input[type="number"]');
    assert.isNotNull(depth);
    depth.value = '4';
    depth.dispatchEvent(new Event('change', { bubbles: true }));
    rest();

    manager.undo();
    // the page shows the panel again for the node undo made
    await view.unmount();
    await show();

    expect(props()).toEqual({ min: 0.2 });
    expect(steps()).toBe(1);
    manager.redo();
    expect(props()).toEqual({ min: 0.2, depth: 4 });
  });
});
