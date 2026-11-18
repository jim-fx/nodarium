import type { GraphManager, GraphState } from '#lib/graph-interface/index.ts';
import { HIGHLIGHT } from '#lib/result-viewer/provenance/highlight.ts';
import { AppSettingTypes, settingsToStore } from '#lib/settings/app-settings.svelte.ts';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { NO_PARENT, type PathLink } from '@nodarium/utils';
import { SvelteSet } from 'svelte/reactivity';
import { describe, expect, it } from 'vitest';
import { nodeTraits } from './provenance';
import { ViewportLink } from './viewport-link.svelte';

const DEFINITIONS = new Map(
  [
    { id: 'p/stem', outputs: ['path'], inputs: { creator: { type: 'creator' } } },
    {
      id: 'p/branch',
      outputs: ['path'],
      inputs: { plant: { type: 'path' }, creator: { type: 'creator' } }
    },
    { id: 'p/output', outputs: [], inputs: { input: { type: 'path' } } }
  ].map((d) => [d.id, d as unknown as NodeDefinition])
);

// stem 1 → branch 2 → output 5
const graph = {
  id: 1,
  nodes: [[1, 'p/stem'], [2, 'p/branch'], [5, 'p/output']].map(([id, type]) => ({
    id,
    type,
    position: [0, 0]
  })),
  edges: [[1, 0, 2, 'plant'], [2, 0, 5, 'input']],
  groups: []
} as unknown as Graph;

const link = (id: number, creator: number, parentId = NO_PARENT): PathLink => ({
  id,
  parentId,
  parentAlpha: 0,
  creator,
  index: 0,
  grown: 1
});
const links = new Map([link(101, 1), link(201, 2, 101)].map((l) => [l.id, l]));

function setup() {
  const settings = $state({ enabled: true });
  const manager = {
    id: 1,
    status: 'idle',
    path: [],
    edges: graph.edges.map(([from, , to]) => [{ id: from }, 0, { id: to }]),
    serialize: () => graph,
    getNode: (id: number) => graph.nodes.find((node) => node.id === id)
  } as unknown as GraphManager;
  const state = $state({
    selectedNodes: new SvelteSet<number>(),
    activeNodeId: -1,
    mouseDown: false,
    camera: { position: [0, 0, 1], animateTo() {} },
    clearSelection() {
      this.selectedNodes.clear();
    },
    exitGroupNode() {}
  });
  const notes: string[] = [];
  const link = new ViewportLink({
    enabled: () => settings.enabled,
    manager: () => manager,
    graphState: () => state as unknown as GraphState,
    result: () => ({ links, tissues: new Set() }),
    traits: nodeTraits((type) => DEFINITIONS.get(type)),
    notify: (message) => notes.push(message)
  });
  return { settings, state, link, notes };
}

describe('the setting', () => {
  it('links viewport and nodes by default, and does not fade the tint', () => {
    expect(settingsToStore(AppSettingTypes).viewportLink).toEqual({
      enabled: true,
      fadeSelection: false
    });
  });
});

describe('ViewportLink', () => {
  it('tints what the selected node made', () => {
    const { state, link } = setup();
    expect(link.highlight).toBeUndefined();
    state.activeNodeId = 2;
    expect(link.highlight?.paths).toEqual(new Map([[201, HIGHLIGHT.focus]]));
  });

  it('points out the node that made the hovered part, and selects it on a click', () => {
    const { state, link } = setup();
    link.onparthover({ kind: 'geometry', pathId: 101 });
    expect(link.highlightedNodeIds).toEqual([1]);
    expect(link.highlight?.paths.get(101)).toBe(HIGHLIGHT.hover);

    link.onpartclick({ kind: 'geometry', pathId: 201 }, { altKey: false });
    expect(state.activeNodeId).toBe(2);
    // with alt, also the nodes of the paths it grows on
    link.onpartclick({ kind: 'geometry', pathId: 201 }, { altKey: true });
    expect([...state.selectedNodes]).toEqual([1]);
  });

  it('shows only the parts of the selection in the local view, and everything again', () => {
    const { state, link, notes } = setup();
    link.toggleLocalView();
    expect(link.localView).toBe(false);
    expect(notes).toEqual(['Select a node or click a part to isolate it']);

    state.activeNodeId = 2;
    link.toggleLocalView();
    expect(link.localView).toBe(true);
    expect(link.highlight?.rest).toBe(HIGHLIGHT.hidden);
    expect(link.highlight?.paths).toEqual(new Map([[201, HIGHLIGHT.none]]));
    // the chain of the local view, from the node to the end of the graph
    expect(link.highlightedNodeIds).toEqual([2, 5]);

    link.toggleLocalView();
    expect(link.localView).toBe(false);
  });

  it('does nothing while the setting is off', () => {
    const { settings, state, link } = setup();
    settings.enabled = false;
    state.activeNodeId = 2;
    link.onparthover({ kind: 'geometry', pathId: 101 });
    expect(link.highlight).toBeUndefined();
    expect(link.highlightedNodeIds).toEqual([]);

    link.onpartclick({ kind: 'geometry', pathId: 101 }, { altKey: false });
    expect(state.activeNodeId).toBe(2);
    link.toggleLocalView();
    expect(link.localView).toBe(false);
  });
});
