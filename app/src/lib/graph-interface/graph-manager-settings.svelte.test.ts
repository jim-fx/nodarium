import type { Graph, NodeDefinition } from '@nodarium/types';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
  const saved: Graph[] = [];
  manager.on('save', (graph) => saved.push(graph));
  const results: Graph[] = [];
  manager.on('result', (graph) => results.push(graph));
  return { manager, saved, results };
}

const types = (manager: GraphManager) => Object.keys(manager.settingTypes).sort();

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
    const { manager, saved } = setup();

    await manager.load(structuredClone(mushroom));
    expect(types(manager)).toEqual(['resolution.curve', 'seed', 'time.t']);
    expect(manager.settings).toEqual({ 'time.t': 0.3, 'resolution.curve': 40, seed: 4 });

    await manager.load(structuredClone(plant));
    expect(types(manager)).toEqual(['resolution.curve', 'seed']);
    expect(manager.settings).toEqual({ 'resolution.curve': 24, seed: 0 });

    manager.flush();
    expect(saved.at(-1)?.id).toBe(2);
    expect(saved.at(-1)?.settings).toEqual({ 'resolution.curve': 24, seed: 0 });
  });

  it('fills settings the graph has no values for with the defaults', async () => {
    const { manager } = setup();

    await manager.load(structuredClone(mushroom));
    await manager.load({ ...structuredClone(plant), settings: undefined });

    expect(manager.settings).toEqual({ 'resolution.curve': 16, seed: 0 });
  });

  it('has no settings while the next graph loads, a value set then is dropped', async () => {
    const { manager, saved } = setup();

    await manager.load(structuredClone(mushroom));
    const loading = manager.load(structuredClone(plant));
    expect(types(manager)).toEqual([]);
    expect(manager.settings).toEqual({});
    manager.setSetting('time.t', 0.9);
    manager.setSetting('resolution.curve', 40);
    await loading;

    expect(manager.settings).toEqual({ 'resolution.curve': 24, seed: 0 });
    manager.flush();
    expect(saved.every((g) => g.id !== 2 || !('time.t' in (g.settings ?? {})))).toBe(true);
  });

  it('only takes values for the settings of the graph', async () => {
    const { manager } = setup();

    await manager.load(structuredClone(plant));
    manager.setSetting('time.t', 0.9);
    manager.setSetting('seed', 7);

    expect(manager.serialize().settings).toEqual({ 'resolution.curve': 24, seed: 7 });
  });

  it('collects the settings of nodes inside groups', async () => {
    const { manager } = setup();

    await manager.load({
      ...structuredClone(plant),
      groups: [{
        id: 0,
        nodes: [{ id: 0, type: 'test/myzel/time', position: [0, 0] }],
        edges: []
      }]
    });

    expect(types(manager)).toEqual(['resolution.curve', 'seed', 'time.t']);
  });

  it('adds the setting of a node added to the graph', async () => {
    const { manager } = setup();

    await manager.load(structuredClone(plant));
    manager.createNode({ type: 'test/plant/leaf', position: [0, 30], props: {} });

    expect(types(manager)).toEqual(['resolution.curve', 'resolution.leaf', 'seed']);
    expect(manager.settings).toEqual({ 'resolution.curve': 24, 'resolution.leaf': 96, seed: 0 });
  });
});

describe('a dragged setting', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('builds every value and becomes one undo step and one save', async () => {
    const { manager, saved, results } = setup();
    await manager.load(structuredClone(plant));
    vi.runAllTimers();
    const steps = manager.history.history.length;
    const saves = saved.length;
    const builds = results.length;

    for (const value of [25, 26, 27]) {
      manager.setSetting('resolution.curve', value);
      vi.advanceTimersByTime(16);
    }
    expect(results.slice(builds).map((g) => g.settings?.['resolution.curve'])).toEqual([
      25,
      26,
      27
    ]);
    expect(manager.history.history.length).toBe(steps);

    vi.runAllTimers();
    expect(manager.history.history.length).toBe(steps + 1);
    expect(saved.length).toBe(saves + 1);
  });

  it('does nothing for the value the setting has', async () => {
    const { manager, saved, results } = setup();
    await manager.load(structuredClone(plant));
    vi.runAllTimers();
    const saves = saved.length;
    const builds = results.length;

    manager.setSetting('resolution.curve', 24);
    vi.runAllTimers();

    expect(results.length).toBe(builds);
    expect(saved.length).toBe(saves);
  });
});
