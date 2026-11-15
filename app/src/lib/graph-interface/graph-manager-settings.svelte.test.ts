import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { createMockNodeRegistry } from './test-utils';

function node(id: string, inputs: NodeDefinition['inputs']): NodeDefinition {
  return {
    id,
    inputs,
    outputs: ['float'],
    meta: {},
    execute: () => new Int32Array()
  } as NodeDefinition;
}

const stem = node('test/plant/stem', {
  resolution: { type: 'integer', value: 16, setting: 'resolution.curve' }
});
const leaf = node('test/plant/leaf', {
  resolution: { type: 'integer', value: 96, setting: 'resolution.leaf' }
});
const time = node('test/myzel/time', {
  t: { type: 'float', value: 0.5, setting: 'time.t' }
});

function setup() {
  const registry = createMockNodeRegistry([stem, leaf, time]);
  registry.loadCollection = async () => [];
  const manager = new GraphManager(registry);
  const emitted: { types: string[]; values: Record<string, unknown> }[] = [];
  manager.on('settings', ({ types, values }) => {
    emitted.push({ types: Object.keys(types).sort(), values });
  });
  const saved: Graph[] = [];
  manager.on('save', (graph) => saved.push(graph));
  return { manager, emitted, saved };
}

const mushroom: Graph = {
  id: 1,
  settings: { 'time.t': 0.3, 'resolution.curve': 40, 'resolution.leaf': 12, seed: 4 },
  nodes: [
    { id: 0, type: 'test/myzel/time', position: [0, 0] },
    { id: 1, type: 'test/plant/stem', position: [30, 0] }
  ],
  edges: [],
  groups: []
};

const plant: Graph = {
  id: 2,
  settings: { 'resolution.curve': 24 },
  nodes: [{ id: 0, type: 'test/plant/stem', position: [0, 0] }],
  edges: [],
  groups: []
};

describe('graph settings', () => {
  it('shows and saves only the settings of the graph loaded last', async () => {
    const { manager, emitted, saved } = setup();

    await manager.load(structuredClone(mushroom));
    expect(emitted.at(-1)).toEqual({
      types: ['resolution.curve', 'time.t'],
      values: { 'time.t': 0.3, 'resolution.curve': 40, seed: 4 }
    });

    await manager.load(structuredClone(plant));
    expect(emitted.at(-1)).toEqual({
      types: ['resolution.curve'],
      values: { 'resolution.curve': 24 }
    });

    manager.flush();
    expect(saved.at(-1)?.id).toBe(2);
    expect(saved.at(-1)?.settings).toEqual({ 'resolution.curve': 24 });
  });

  it('fills settings the graph has no values for with the defaults', async () => {
    const { manager, emitted } = setup();

    await manager.load(structuredClone(mushroom));
    await manager.load({ ...structuredClone(plant), settings: undefined });

    expect(emitted.at(-1)?.values).toEqual({ 'resolution.curve': 16 });
  });

  it('clears the settings while the next graph loads and ignores the old ones', async () => {
    const { manager, emitted, saved } = setup();

    await manager.load(structuredClone(mushroom));
    const loading = manager.load(structuredClone(plant));
    expect(emitted.at(-1)).toEqual({ types: [], values: {} });
    manager.setSettings(structuredClone(mushroom.settings!));
    await loading;

    manager.setSettings({ ...mushroom.settings, 'resolution.curve': 24 });
    manager.flush();
    expect(manager.serialize().settings).toEqual({ 'resolution.curve': 24, seed: 4 });
    expect(saved.every((g) => g.id !== 2 || !('time.t' in (g.settings ?? {})))).toBe(true);
  });

  it('collects the settings of nodes inside groups', async () => {
    const { manager, emitted } = setup();

    await manager.load({
      ...structuredClone(plant),
      groups: [{
        id: 0,
        nodes: [{ id: 0, type: 'test/myzel/time', position: [0, 0] }],
        edges: []
      }]
    });

    expect(emitted.at(-1)?.types).toEqual(['resolution.curve', 'time.t']);
  });

  it('adds the setting of a node added to the graph', async () => {
    const { manager, emitted } = setup();

    await manager.load(structuredClone(plant));
    manager.createNode({ type: 'test/plant/leaf', position: [0, 30], props: {} });

    expect(emitted.at(-1)).toEqual({
      types: ['resolution.curve', 'resolution.leaf'],
      values: { 'resolution.curve': 24, 'resolution.leaf': 96 }
    });
  });
});
