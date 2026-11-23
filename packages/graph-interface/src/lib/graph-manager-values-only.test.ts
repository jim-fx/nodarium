import type { Graph } from '@nodarium/runtime';
import { describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { createMockNodeRegistry, mockFloatInputNode, mockFloatOutputNode } from './test-utils';

const graph: Graph = {
  id: 1,
  nodes: [
    { id: 1, type: 'test/node/input', position: [0, 0], props: { value: 0.2 } },
    { id: 2, type: 'test/node/input', position: [30, 0] },
    { id: 3, type: 'test/node/output', position: [60, 0] }
  ],
  edges: [[1, 0, 2, 'value'], [2, 0, 3, 'input']],
  frames: [{ id: 4, label: 'Stem', nodes: [1, 2] }],
  groups: []
};

async function load({ valuesOnly = true } = {}) {
  const manager = new GraphManager(
    createMockNodeRegistry([mockFloatInputNode, mockFloatOutputNode])
  );
  manager.valuesOnly = valuesOnly;
  await manager.load(structuredClone(graph));
  return manager;
}

const structure = (g: Graph) => ({
  nodes: g.nodes.map(({ id, type, position }) => ({ id, type, position })),
  edges: g.edges,
  frames: g.frames,
  groups: g.groups
});

describe('GraphManager.valuesOnly', () => {
  it('takes a changed value and builds with it', async () => {
    const manager = await load();
    const results: Graph[] = [];
    manager.on('result', (g) => results.push(g));

    const node = manager.getNode(2)!;
    node.props = { ...node.props, value: 0.9 };
    manager.commit();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(manager.serialize().nodes.find((n) => n.id === 2)?.props).toEqual({ value: 0.9 });
    expect(results.at(-1)?.nodes.find((n) => n.id === 2)?.props).toEqual({ value: 0.9 });
  });

  it('undoes every change of the structure', async () => {
    const manager = await load();
    const before = structure(manager.serialize());

    manager.createNode({ type: 'test/node/input', position: [90, 0], props: {} });
    manager.removeNode(manager.getNode(1)!);
    manager.commit();
    manager.removeEdge(manager.edges[0]);
    manager.commit();
    manager.createEdge(manager.getNode(1)!, 0, manager.getNode(3)!, 'input');
    manager.muteNode(2);
    manager.createFrame([3], 'More');
    manager.removeFrame(4);
    manager.groupNodes([1, 2]);
    const moved = manager.getNode(3)!;
    moved.position = [500, 500];
    manager.commit({ build: false });

    expect(structure(manager.serialize())).toEqual(before);
  });

  it('keeps the values changed before a change of the structure was undone', async () => {
    const manager = await load();
    const node = manager.getNode(1)!;
    node.props = { value: 0.7 };
    manager.commit();

    manager.removeNode(manager.getNode(3)!);
    manager.commit();

    expect(manager.serialize().nodes.find((n) => n.id === 1)?.props).toEqual({ value: 0.7 });
    expect(manager.nodes.size).toBe(3);
  });

  it('changes the structure as before without the mode', async () => {
    const manager = await load({ valuesOnly: false });
    manager.removeNode(manager.getNode(3)!);
    manager.commit();
    expect(manager.serialize().nodes.map((n) => n.id)).toEqual([1, 2]);
  });
});
