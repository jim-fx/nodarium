import type { Graph, SerializedNode } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import {
  CURRENT_GRAPH_VERSION,
  GraphVersionError,
  mapInput,
  migrateGraph,
  MIGRATIONS,
  removeInputs,
  renameInputs,
  renameNodeType,
  replaceNodes
} from './index';

const fixtures = import.meta.glob<Graph>('./fixtures/v*.json', { eager: true, import: 'default' });

function fixture(version: number) {
  return structuredClone(fixtures[`./fixtures/v${version}.json`]);
}

function node(id: number, type: string, props: SerializedNode['props'] = {}): SerializedNode {
  return { id, type: type as SerializedNode['type'], props, position: [0, 0] };
}

function graph(nodes: SerializedNode[], edges: Graph['edges'] = []): Graph {
  return { id: 1, version: CURRENT_GRAPH_VERSION, nodes, edges, groups: [] };
}

describe('migrateGraph', () => {
  it('has a fixture for every version', () => {
    for (let version = 0; version <= CURRENT_GRAPH_VERSION; version++) {
      expect(fixtures[`./fixtures/v${version}.json`], `fixtures/v${version}.json`).toBeDefined();
    }
  });

  it('numbers migrations one after another', () => {
    expect(MIGRATIONS.map((m) => m.version)).toEqual(MIGRATIONS.map((_, i) => i + 1));
  });

  it('migrates the fixture of every version to the current one', () => {
    const current = fixture(CURRENT_GRAPH_VERSION);
    for (let version = 0; version <= CURRENT_GRAPH_VERSION; version++) {
      expect(migrateGraph(fixture(version)), `fixtures/v${version}.json`).toEqual(current);
    }
  });

  it('leaves a current graph as it is', () => {
    const current = fixture(CURRENT_GRAPH_VERSION);
    expect(migrateGraph(current)).toBe(current);
  });

  it('does not change the graph it migrates', () => {
    const old = fixture(0);
    const copy = structuredClone(old);
    migrateGraph(old);
    expect(old).toEqual(copy);
  });

  it('rejects a graph that is newer than the app', () => {
    const newer = { ...fixture(CURRENT_GRAPH_VERSION), version: CURRENT_GRAPH_VERSION + 1 };
    expect(() => migrateGraph(newer)).toThrow(GraphVersionError);
  });
});

describe('renameInputs', () => {
  it('moves values, exposed inputs and edges', () => {
    const g = graph(
      [node(0, 'a/b/branch', { lowestBranch: 0.3, amount: 4 }), node(1, 'a/b/random')],
      [[1, 0, 0, 'lowestBranch']]
    );
    g.nodes[0].exposed = ['lowestBranch'];
    renameInputs(g, 'a/b/branch', { lowestBranch: 'lowest_branch' });
    expect(g.nodes[0].props).toEqual({ lowest_branch: 0.3, amount: 4 });
    expect(g.nodes[0].exposed).toEqual(['lowest_branch']);
    expect(g.edges).toEqual([[1, 0, 0, 'lowest_branch']]);
  });

  it('keeps a value saved under the new name', () => {
    const g = graph([node(0, 'a/b/branch', { lowestBranch: 0.3, lowest_branch: 0.5 })]);
    renameInputs(g, 'a/b/branch', { lowestBranch: 'lowest_branch' });
    expect(g.nodes[0].props).toEqual({ lowest_branch: 0.5 });
  });

  it('leaves other node types and inputs of the same name alone', () => {
    const g = graph(
      [node(0, 'a/b/branch'), node(1, 'a/b/other', { lowestBranch: 1 })],
      [[0, 0, 1, 'lowestBranch']]
    );
    renameInputs(g, 'a/b/branch', { lowestBranch: 'lowest_branch' });
    expect(g.nodes[1].props).toEqual({ lowestBranch: 1 });
    expect(g.edges).toEqual([[0, 0, 1, 'lowestBranch']]);
  });

  it('renames inside groups', () => {
    const g = graph([]);
    g.groups = [{
      id: 0,
      nodes: [node(0, 'a/b/branch', { lowestBranch: 0.3 }), node(1, 'a/b/random')],
      edges: [[1, 0, 0, 'lowestBranch']]
    }];
    renameInputs(g, 'a/b/branch', { lowestBranch: 'lowest_branch' });
    expect(g.groups[0].nodes[0].props).toEqual({ lowest_branch: 0.3 });
    expect(g.groups[0].edges).toEqual([[1, 0, 0, 'lowest_branch']]);
  });
});

describe('renameNodeType', () => {
  it('renames nodes on every level', () => {
    const g = graph([node(0, 'a/b/old')]);
    g.groups = [{ id: 0, nodes: [node(0, 'a/b/old')], edges: [] }];
    renameNodeType(g, 'a/b/old', 'a/b/new');
    expect(g.nodes[0].type).toBe('a/b/new');
    expect(g.groups[0].nodes[0].type).toBe('a/b/new');
  });
});

describe('mapInput', () => {
  it('converts saved values only', () => {
    const g = graph([node(0, 'a/b/n', { angle: 90 }), node(1, 'a/b/n')]);
    mapInput(g, 'a/b/n', 'angle', (v) => (v as number) * Math.PI / 180);
    expect(g.nodes[0].props?.angle).toBeCloseTo(Math.PI / 2);
    expect(g.nodes[1].props).toEqual({});
  });
});

describe('removeInputs', () => {
  it('removes values, exposed inputs and edges', () => {
    const g = graph(
      [node(0, 'a/b/n', { gone: 1, kept: 2 }), node(1, 'a/b/random')],
      [[1, 0, 0, 'gone'], [1, 0, 0, 'kept']]
    );
    g.nodes[0].exposed = ['gone', 'kept'];
    removeInputs(g, 'a/b/n', ['gone']);
    expect(g.nodes[0].props).toEqual({ kept: 2 });
    expect(g.nodes[0].exposed).toEqual(['kept']);
    expect(g.edges).toEqual([[1, 0, 0, 'kept']]);
  });
});

describe('replaceNodes', () => {
  function branchGraph() {
    return graph(
      [
        node(0, 'a/b/stem'),
        node(1, 'a/b/branch', { amount: 6, length: 2 }),
        node(2, 'a/b/output'),
        node(3, 'a/b/random')
      ],
      [[0, 0, 1, 'plant'], [3, 0, 1, 'length'], [1, 0, 2, 'input']]
    );
  }

  it('rewires edges onto the new nodes', () => {
    const g = replaceNodes(branchGraph(), 'a/b/branch', (old, { nextId }) => {
      const points = nextId();
      const branch = nextId();
      return {
        nodes: [
          node(points, 'a/b/points_on_path', { amount: old.props!.amount }),
          node(branch, 'a/b/branch_from_points', { length: old.props!.length })
        ],
        edges: [[points, 0, branch, 'points']],
        inputs: { plant: [[points, 'plant'], [branch, 'plant']], length: [[branch, 'length']] },
        outputs: { 0: [branch, 0] }
      };
    });

    expect(g.nodes.map((n) => [n.id, n.type])).toEqual([
      [0, 'a/b/stem'],
      [4, 'a/b/points_on_path'],
      [5, 'a/b/branch_from_points'],
      [2, 'a/b/output'],
      [3, 'a/b/random']
    ]);
    expect(g.nodes[1].props).toEqual({ amount: 6 });
    expect(g.edges).toEqual([
      [0, 0, 4, 'plant'],
      [0, 0, 5, 'plant'],
      [3, 0, 5, 'length'],
      [5, 0, 2, 'input'],
      [4, 0, 5, 'points']
    ]);
  });

  it('drops edges of sockets without a replacement', () => {
    const g = replaceNodes(branchGraph(), 'a/b/branch', (_, { nextId }) => ({
      nodes: [node(nextId(), 'a/b/new')]
    }));
    expect(g.edges).toEqual([]);
  });

  it('can replace a node with a group', () => {
    const g = replaceNodes(branchGraph(), 'a/b/branch', (_, { nextId, addGroup }) => {
      const groupId = addGroup({ name: 'branch', nodes: [], edges: [] });
      const instance = nextId();
      return {
        nodes: [node(instance, '__internal/group/instance', { groupId })],
        inputs: { plant: [[instance, 'in_0']] },
        outputs: { 0: [instance, 0] }
      };
    });
    expect(g.groups.map((group) => group.id)).toEqual([0]);
    expect(g.nodes[1]).toMatchObject({ id: 4, props: { groupId: 0 } });
    expect(g.edges).toEqual([[0, 0, 4, 'in_0'], [4, 0, 2, 'input']]);
  });

  it('keeps a node the replacement returns nothing for', () => {
    const g = replaceNodes(branchGraph(), 'a/b/branch', () => undefined);
    expect(g).toEqual(branchGraph());
  });
});
