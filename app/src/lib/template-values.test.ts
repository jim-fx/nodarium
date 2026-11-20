import { decodeGraph, graphLink, graphLinkData } from '@nodarium/runtime';
import { loadTemplate } from '@nodarium/runtime/templates';
import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { changedValues, editorGraph, withValues } from './template-values';

const template: Graph = {
  id: 3,
  meta: { title: 'Stem' },
  settings: { seed: 2 },
  nodes: [
    { id: 1, type: 'max/plantarium/stem', position: [0, 0], props: { amount: 3, length: 0.5 } },
    { id: 2, type: 'max/plantarium/output', position: [30, 0] },
    { id: 5, type: '__internal/group/instance', position: [0, 30], props: { groupId: 9 } }
  ],
  edges: [[1, 0, 2, 'input']],
  groups: [{ id: 9, nodes: [{ id: 6, type: 'max/core/float', position: [0, 0] }], edges: [] }]
};

describe('changedValues', () => {
  it('holds only the values that differ from the template, in groups too', () => {
    const edited = structuredClone(template);
    edited.nodes[0].props = { amount: 3, length: 0.8 };
    edited.nodes[1].props = { curve: [0, 1] };
    edited.groups![0].nodes[0].props = { value: 2 };

    expect(changedValues(template, edited)).toEqual({
      1: { length: 0.8 },
      2: { curve: [0, 1] },
      6: { value: 2 }
    });
    expect(changedValues(template, structuredClone(template))).toEqual({});
  });
});

describe('withValues', () => {
  it('leaves the template as it is', () => {
    const graph = withValues(template, { 1: { length: 0.8 } });
    expect(graph.nodes[0].props).toEqual({ amount: 3, length: 0.8 });
    expect(template.nodes[0].props).toEqual({ amount: 3, length: 0.5 });
  });
});

describe('the link of the editor', () => {
  it('opens the template with the changed values, the seed and the time', async () => {
    const fern = (await loadTemplate('fern')) as Graph;
    const node = fern.nodes.find((n) => n.props && Object.keys(n.props).length)!;
    const [key] = Object.keys(node.props!);
    const values = { [node.id]: { [key]: 0.123 } };

    const link = await graphLink(
      editorGraph(fern, values, { title: 'Fern', seed: 7, time: 0.3 }),
      'http://localhost'
    );
    const opened = (await decodeGraph(graphLinkData(link)!)) as Graph;

    expect(opened.meta?.title).toBe('Fern');
    expect(opened.settings).toMatchObject({ seed: 7, 'time.t': 0.3 });
    expect(opened.nodes.find((n) => n.id === node.id)?.props?.[key]).toBe(0.123);
    const others = (g: Graph) => g.nodes.filter((n) => n.id !== node.id);
    expect(others(opened)).toEqual(others(fern));
    expect(opened.edges).toEqual(fern.edges);
    expect(opened.groups).toEqual(fern.groups);
    expect(opened.frames).toEqual(fern.frames);
  });
});
