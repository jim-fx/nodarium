import type { Graph, SerializedNode } from '@nodarium/types';
import { describe, expect, it, vi } from 'vitest';
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

/** The graph as the migrations up to `version` leave it, for what later ones change again */
function migrateTo(version: number, old: Graph): Graph {
  let migrated: Graph = structuredClone(old);
  for (const migration of MIGRATIONS) {
    if (migration.version <= (old.version ?? 0) || migration.version > version) continue;
    migrated = migration.migrate(migrated);
    migrated.version = migration.version;
  }
  return migrated;
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
    // group ids don't share numbers with node ids either
    expect(g.groups.map((group) => group.id)).toEqual([4]);
    expect(g.nodes[1]).toMatchObject({ id: 5, props: { groupId: 4 } });
    expect(g.edges).toEqual([[0, 0, 5, 'in_0'], [5, 0, 2, 'input']]);
  });

  it('gives new nodes ids no level or group uses yet', () => {
    // the group was made before nodes 5 to 7 of the top level, so its ids are lower
    const g = graph([
      node(0, 'a/b/stem'),
      node(1, 'a/b/output'),
      node(5, 'a/b/x'),
      node(6, 'a/b/x'),
      node(7, 'a/b/x')
    ]);
    g.groups = [
      { id: 2, nodes: [node(3, 'a/b/branch'), node(4, 'a/b/y')], edges: [] }
    ] as unknown as Graph['groups'];
    replaceNodes(g, 'a/b/branch', (_, { nextId, addGroup }) => ({
      nodes: [node(nextId(), 'a/b/new'), node(nextId(), 'a/b/new')],
      inputs: {},
      outputs: { 0: [addGroup({ name: 'n', nodes: [], edges: [] }), 0] }
    }));
    const ids = [
      ...g.nodes.map((n) => n.id),
      ...g.groups.flatMap((group) => [group.id, ...group.nodes.map((n) => n.id)])
    ];
    expect(new Set(ids).size).toBe(ids.length);
    expect(g.groups[0].nodes.map((n) => n.id)).toEqual([8, 9, 4]);
  });

  it('keeps a node the replacement returns nothing for', () => {
    const g = replaceNodes(branchGraph(), 'a/b/branch', () => undefined);
    expect(g).toEqual(branchGraph());
  });
});

describe('version 3: the cap is split into cap and pose nodes', () => {
  const v2 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 2
  });
  const types = (g: Graph) => g.nodes.map((n) => n.type);
  const byType = (g: Graph, type: string) => g.nodes.find((n) => n.type === type)!;

  it('turns the old defaults into the new nodes the templates use', () => {
    const migrated = migrateTo(
      3,
      v2(
        [node(1, 'max/plantarium/stem'), node(2, 'max/myzel/cap'), node(3, 'max/myzel/strands')],
        [[1, 0, 2, 'plant'], [2, 0, 3, 'plant']]
      )
    );
    expect(migrated.version).toBe(3);
    expect(types(migrated)).toEqual([
      'max/plantarium/stem',
      'max/myzel/cap',
      'max/plantarium/curve',
      'max/plantarium/fill_gaps',
      'max/myzel/margin_curl',
      'max/myzel/strands'
    ]);
    const curve = byType(migrated, 'max/plantarium/curve');
    expect(curve.props).toEqual({ curve: [0, 1, 0.6, 0.6, 1, 0.15], min: 0, max: 0.008 });
    const gaps = byType(migrated, 'max/plantarium/fill_gaps');
    const curl = byType(migrated, 'max/myzel/margin_curl');
    expect(gaps.props).toEqual({ spacing: 0.5 });
    expect(curl.props).toEqual({ amount: 0.2 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, 2, 'plant'],
        [curve.id, 0, 2, 'thickness'],
        [2, 0, gaps.id, 'plant'],
        [gaps.id, 0, curl.id, 'cap'],
        [2, 1, curl.id, 'hinge'],
        [curl.id, 0, 3, 'plant']
      ])
    );
    expect(migrated.edges).toHaveLength(6);
  });

  it('moves values and wired inputs to the node that has them now', () => {
    const cap = node(2, 'max/myzel/cap', {
      radius: 0.03,
      rays: 20,
      thickness: 0.012,
      thickness_curve: [0, 1, 1, 0.5],
      margin_curl: 0.4,
      waves: 5,
      wave_amplitude: 0.2,
      wave_irregularity: 0.6,
      wave_profile: [0, 0, 1, 1],
      spacing: 0.8
    });
    const migrated = migrateTo(
      3,
      v2(
        [
          node(1, 'max/plantarium/stem'),
          cap,
          node(4, 'max/plantarium/float'),
          node(5, 'max/myzel/membrane')
        ],
        [[1, 0, 2, 'plant'], [4, 0, 2, 'opening'], [2, 2, 5, 'a'], [2, 0, 5, 'b']]
      )
    );
    expect(byType(migrated, 'max/myzel/cap').props).toEqual({ radius: 0.03, rays: 20 });
    const [thickness, amplitude] = migrated.nodes.filter((n) => n.type === 'max/plantarium/curve');
    expect(thickness.props).toEqual({ curve: [0, 1, 1, 0.5], min: 0, max: 0.012 });
    expect(amplitude.props).toEqual({ curve: [0, 0, 1, 1], min: 0, max: 0.2 });
    const gaps = byType(migrated, 'max/plantarium/fill_gaps');
    const waves = byType(migrated, 'max/myzel/margin_waves');
    const curl = byType(migrated, 'max/myzel/margin_curl');
    const open = byType(migrated, 'max/myzel/cap_open');
    expect(waves.props).toEqual({ waves: 5, irregularity: 0.6 });
    expect(curl.props).toEqual({ amount: 0.4 });
    // the opening still comes from the float node, rays and margin from the last pose node
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [thickness.id, 0, 2, 'thickness'],
        [2, 0, gaps.id, 'plant'],
        [gaps.id, 0, waves.id, 'cap'],
        [amplitude.id, 0, waves.id, 'amplitude'],
        [waves.id, 0, curl.id, 'cap'],
        [curl.id, 0, open.id, 'cap'],
        [4, 0, open.id, 'opening'],
        [open.id, 1, 5, 'a'],
        [open.id, 0, 5, 'b']
      ])
    );
    for (const pose of [waves, curl, open]) {
      expect(migrated.edges).toContainEqual([2, 1, pose.id, 'hinge']);
    }
  });

  it('leaves out pose nodes that would change nothing', () => {
    const migrated = migrateTo(
      3,
      v2(
        [node(2, 'max/myzel/cap', { spacing: 0, margin_curl: 0 }), node(5, 'max/myzel/membrane')],
        [[2, 2, 5, 'a']]
      )
    );
    expect(types(migrated)).toEqual([
      'max/myzel/cap',
      'max/plantarium/curve',
      'max/myzel/membrane'
    ]);
    expect(migrated.edges).toContainEqual([2, 2, 5, 'a']);
  });

  it('gives the margin of the added rays when only fill_gaps poses the cap', () => {
    const migrated = migrateTo(
      3,
      v2([node(2, 'max/myzel/cap', { margin_curl: 0 }), node(5, 'max/myzel/membrane')], [[
        2,
        2,
        5,
        'a'
      ]])
    );
    const open = byType(migrated, 'max/myzel/cap_open');
    expect(open.props).toEqual({});
    expect(migrated.edges).toContainEqual([open.id, 1, 5, 'a']);
  });

  it('splits caps inside groups, with ids no other level uses', () => {
    // nodes 3 and 4 of the top level were made after the group, its own ids are lower
    const old = v2([
      node(9, 'max/plantarium/output'),
      node(3, 'max/plantarium/float'),
      node(4, 'max/plantarium/float')
    ]);
    old.groups = [
      {
        id: 0,
        nodes: [node(2, 'max/myzel/cap', { margin_curl: 0, spacing: 0, opening: 0.3 })],
        edges: []
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateTo(3, old);
    const group = migrated.groups![0];
    const ids = [
      ...migrated.nodes.map((n) => n.id),
      ...migrated.groups!.flatMap((g) => [g.id, ...g.nodes.map((n) => n.id)])
    ];
    expect(new Set(ids).size).toBe(ids.length);
    expect(group.nodes.map((n) => n.type)).toEqual([
      'max/myzel/cap',
      'max/plantarium/curve',
      'max/myzel/cap_open'
    ]);
  });
});

describe('version 4: a random node into the thickness of strands varies it', () => {
  const v3 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 3
  });
  const byType = (g: Graph, type: string) => g.nodes.filter((n) => n.type === type);
  const strands = (g: Graph) => byType(g, 'max/myzel/strands')[0];

  it('only drops a variation of 0', () => {
    const migrated = migrateGraph(
      v3([node(1, 'max/myzel/strands', { thickness: 0.001, thickness_variation: 0 })])
    );
    expect(migrated.nodes.map((n) => n.type)).toEqual(['max/myzel/strands']);
    expect(strands(migrated).props).toEqual({ thickness: 0.001 });
  });

  it('turns a plain variation into a random node from the thinnest to the full thickness', () => {
    const migrated = migrateGraph(
      v3(
        [
          node(1, 'max/plantarium/stem'),
          node(2, 'max/myzel/strands', { thickness_variation: 0.5 })
        ],
        [[1, 0, 2, 'plant']]
      )
    );
    const [random] = byType(migrated, 'max/plantarium/random');
    expect(random.props).toEqual({ min: 0.00025, max: 0.0005, uniform: 1 });
    expect(strands(migrated).props).toEqual({});
    expect(migrated.edges).toEqual([[1, 0, 2, 'plant'], [random.id, 0, 2, 'thickness']]);
  });

  it('multiplies a wired thickness by a random share', () => {
    const migrated = migrateGraph(
      v3(
        [
          node(1, 'max/plantarium/float'),
          node(2, 'max/myzel/strands', { thickness_variation: 0.25 })
        ],
        [[1, 0, 2, 'thickness']]
      )
    );
    const [random] = byType(migrated, 'max/plantarium/random');
    const [math] = byType(migrated, 'max/plantarium/math');
    expect(random.props).toEqual({ min: 0.75, max: 1, uniform: 1 });
    expect(math.props).toMatchObject({ op_type: 2 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, math.id, 'a'],
        [random.id, 0, math.id, 'b'],
        [math.id, 0, 2, 'thickness']
      ])
    );
    expect(migrated.edges).toHaveLength(3);
  });

  it('builds the old formula from math nodes for a wired variation', () => {
    const migrated = migrateGraph(
      v3(
        [node(1, 'max/plantarium/float'), node(2, 'max/myzel/strands', { thickness: 0.001 })],
        [[1, 0, 2, 'thickness_variation']]
      )
    );
    const [thinner, kept, product] = byType(migrated, 'max/plantarium/math');
    expect(product.props).toMatchObject({ op_type: 2, a: 0.001 });
    expect(kept.props).toMatchObject({ op_type: 1, a: 1 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, thinner.id, 'a'],
        [thinner.id, 0, kept.id, 'b'],
        [kept.id, 0, product.id, 'b'],
        [product.id, 0, 2, 'thickness']
      ])
    );
    expect(migrated.edges.some((e) => e[3] === 'thickness_variation')).toBe(false);
  });
});

describe('version 5: hyphae_to_mesh takes paths only, join_meshes joins its mesh with the others', () => {
  const v4 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 4
  });
  const byType = (g: Graph, type: string) => g.nodes.filter((n) => n.type === type);
  const HYPHAE = 'max/myzel/hyphae_to_mesh';
  const JOIN_MESHES = 'max/myzel/join_meshes';

  it('joins the meshes of lamellae behind hyphae_to_mesh and moves the mesh output up', () => {
    const migrated = migrateTo(
      10,
      v4(
        [
          node(1, 'max/myzel/strands'),
          node(2, HYPHAE, { fill: 1.5 }),
          node(3, 'max/myzel/lamellae'),
          node(4, 'max/myzel/output')
        ],
        [[1, 0, 2, 'hyphae'], [3, 1, 2, 'meshes'], [2, 0, 4, 'mesh']]
      )
    );
    const [join] = byType(migrated, JOIN_MESHES);
    expect(join.position).toEqual([30, 0]);
    expect(byType(migrated, HYPHAE)[0].props).toEqual({ fill: 1.5 });
    expect(migrated.edges).toEqual([
      [1, 0, 2, 'paths'],
      [2, 0, join.id, 'a'],
      [3, 0, join.id, 'b'],
      [join.id, 0, 4, 'mesh']
    ]);
  });

  it('takes the meshes of a join node in its order, after the paths', () => {
    const migrated = migrateTo(
      10,
      v4(
        [
          node(1, HYPHAE),
          node(2, 'max/plantarium/join'),
          node(3, 'max/myzel/lamellae'),
          node(4, 'max/myzel/membrane', { strands: 100 }),
          node(5, 'max/myzel/output')
        ],
        [[3, 1, 2, 'a'], [4, 1, 2, 'b'], [2, 0, 1, 'meshes'], [1, 0, 5, 'field']]
      )
    );
    expect(migrated.nodes.map((n) => n.type)).not.toContain('max/plantarium/join');
    const [join] = byType(migrated, JOIN_MESHES);
    expect(migrated.edges).toEqual([
      [1, 0, join.id, 'a'],
      [3, 0, join.id, 'b'],
      [4, 0, join.id, 'c'],
      [join.id, 0, 5, 'mesh']
    ]);
    expect(byType(migrated, 'max/myzel/membrane')[0].props).toEqual({});
  });

  it('chains join nodes when there are more than four meshes', () => {
    const lamellae = [3, 4, 5, 6, 7].map((id) => node(id, 'max/myzel/lamellae'));
    const migrated = migrateTo(
      10,
      v4(
        [node(1, HYPHAE), node(2, 'max/myzel/output'), ...lamellae],
        [
          ...lamellae.map((n): Graph['edges'][number] => [n.id, 1, 1, 'meshes']),
          [1, 0, 2, 'field']
        ]
      )
    );
    const [first, second] = byType(migrated, JOIN_MESHES);
    expect(migrated.edges).toEqual([
      [1, 0, first.id, 'a'],
      [3, 0, first.id, 'b'],
      [4, 0, first.id, 'c'],
      [5, 0, first.id, 'd'],
      [first.id, 0, second.id, 'a'],
      [6, 0, second.id, 'b'],
      [7, 0, second.id, 'c'],
      [second.id, 0, 2, 'mesh']
    ]);
  });

  it('joins the mesh of a sheet whose path output went into loose', () => {
    const migrated = migrateTo(
      10,
      v4(
        [
          node(1, 'max/myzel/strands'),
          node(2, HYPHAE),
          node(3, 'max/myzel/lamellae'),
          node(4, 'max/myzel/output')
        ],
        [[1, 0, 2, 'hyphae'], [3, 0, 2, 'loose'], [2, 0, 4, 'mesh']]
      )
    );
    const [join] = byType(migrated, JOIN_MESHES);
    expect(migrated.edges).toEqual([
      [1, 0, 2, 'paths'],
      [2, 0, join.id, 'a'],
      [3, 0, join.id, 'b'],
      [join.id, 0, 4, 'mesh']
    ]);
  });

  it('joins the mesh of a sheet whose path output went through a join into loose or hyphae', () => {
    const migrated = migrateTo(
      10,
      v4(
        [
          node(1, 'max/myzel/strands'),
          node(2, HYPHAE),
          node(3, 'max/myzel/lamellae'),
          node(4, 'max/myzel/membrane'),
          node(5, 'max/plantarium/join'),
          node(6, 'max/myzel/output')
        ],
        [
          [1, 0, 5, 'a'],
          [3, 0, 5, 'b'],
          [4, 0, 2, 'loose'],
          [5, 0, 2, 'hyphae'],
          [4, 1, 2, 'meshes'],
          [2, 0, 6, 'field']
        ]
      )
    );
    const [join] = byType(migrated, JOIN_MESHES);
    expect(migrated.edges).toEqual([
      [1, 0, 5, 'a'],
      [5, 0, 2, 'paths'],
      [2, 0, join.id, 'a'],
      [4, 0, join.id, 'b'],
      [3, 0, join.id, 'c'],
      [join.id, 0, 6, 'mesh']
    ]);
  });

  it('expands a join that also feeds other nodes and keeps it for them', () => {
    const migrated = migrateTo(
      10,
      v4(
        [
          node(1, HYPHAE),
          node(2, 'max/plantarium/join'),
          node(3, 'max/myzel/lamellae'),
          node(4, 'max/myzel/strands'),
          node(5, 'max/myzel/output'),
          node(6, 'max/plantarium/output')
        ],
        [
          [3, 1, 2, 'a'],
          [4, 0, 2, 'b'],
          [2, 0, 1, 'meshes'],
          [2, 0, 6, 'input'],
          [1, 0, 5, 'field']
        ]
      )
    );
    const [join] = byType(migrated, JOIN_MESHES);
    expect(byType(migrated, 'max/plantarium/join')).toHaveLength(1);
    expect(migrated.edges).toEqual([
      [4, 0, 2, 'b'],
      [2, 0, 6, 'input'],
      [1, 0, join.id, 'a'],
      [3, 0, join.id, 'b'],
      [join.id, 0, 5, 'mesh']
    ]);
  });

  it('leaves hyphae_to_mesh without meshes alone but for the input names', () => {
    const migrated = migrateTo(
      10,
      v4(
        [node(1, 'max/myzel/strands'), node(2, HYPHAE, { loose: 1 }), node(3, 'max/myzel/strands')],
        [[1, 0, 2, 'hyphae'], [3, 0, 2, 'loose']]
      )
    );
    expect(byType(migrated, JOIN_MESHES)).toEqual([]);
    expect(migrated.edges).toEqual([[1, 0, 2, 'paths']]);
    expect(byType(migrated, HYPHAE)[0].props).toEqual({});
  });

  it('drops the paths of lamellae and membrane', () => {
    const migrated = migrateTo(
      10,
      v4(
        [node(1, 'max/myzel/lamellae'), node(2, 'max/myzel/membrane'), node(3, 'max/myzel/output')],
        [[1, 0, 3, 'field'], [2, 0, 3, 'field'], [2, 1, 3, 'field']]
      )
    );
    expect(migrated.edges).toEqual([[2, 0, 3, 'mesh']]);
  });

  it('lets the paths pass where set_blend was', () => {
    const migrated = migrateTo(
      10,
      v4(
        [node(1, 'max/myzel/strands'), node(2, 'max/myzel/set_blend'), node(3, HYPHAE)],
        [[1, 0, 2, 'plant'], [2, 0, 3, 'hyphae']]
      )
    );
    expect(migrated.nodes.map((n) => n.id)).toEqual([1, 3]);
    expect(migrated.edges).toEqual([[1, 0, 3, 'paths']]);
  });

  it('migrates inside groups with ids no other level uses', () => {
    const old = v4([node(9, 'max/myzel/output')]);
    old.groups = [
      {
        id: 0,
        nodes: [node(1, HYPHAE), node(2, 'max/myzel/lamellae')],
        edges: [[2, 1, 1, 'meshes']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateTo(10, old);
    const group = migrated.groups![0];
    const join = group.nodes.find((n) => n.type === JOIN_MESHES)!;
    expect(join.id).toBe(10);
    expect(group.edges).toEqual([[1, 0, 10, 'a'], [2, 0, 10, 'b']]);
  });
});

describe('version 6: set_color takes paths only, set_mesh_color paints meshes', () => {
  const v5 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 5
  });
  const SET_COLOR = 'max/myzel/set_color';
  const SET_MESH_COLOR = 'max/myzel/set_mesh_color';
  const pink = [217, 160, 150];

  it('renames the input of a set_color that only takes paths', () => {
    const migrated = migrateTo(
      10,
      v5(
        [
          node(1, 'max/myzel/strands'),
          node(2, SET_COLOR, { color: pink }),
          node(3, 'max/myzel/hyphae_to_mesh')
        ],
        [[1, 0, 2, 'plant'], [2, 0, 3, 'paths']]
      )
    );
    expect(migrated.nodes.map((n) => n.type)).not.toContain(SET_MESH_COLOR);
    expect(migrated.nodes.find((n) => n.id === 2)!.props).toEqual({ color: pink });
    expect(migrated.edges).toEqual([[1, 0, 2, 'paths'], [2, 0, 3, 'paths']]);
  });

  it('turns a set_color that only paints a mesh into set_mesh_color, its mesh is output 0', () => {
    const migrated = migrateTo(
      10,
      v5(
        [
          node(1, 'max/myzel/lamellae'),
          node(2, SET_COLOR, { color: pink }),
          node(3, 'max/myzel/join_meshes')
        ],
        [[1, 0, 2, 'mesh'], [2, 1, 3, 'b']]
      )
    );
    expect(migrated.nodes.map((n) => [n.id, n.type])).toEqual([
      [1, 'max/myzel/lamellae'],
      [2, SET_MESH_COLOR],
      [3, 'max/myzel/join_meshes']
    ]);
    expect(migrated.nodes[1].props).toEqual({ color: pink });
    expect(migrated.edges).toEqual([[1, 0, 2, 'mesh'], [2, 0, 3, 'b']]);
  });

  it('keeps a color wired into a set_color that only paints a mesh', () => {
    const migrated = migrateTo(
      10,
      v5(
        [node(1, 'max/myzel/mix_color'), node(2, SET_COLOR), node(3, 'max/myzel/join_meshes')],
        [[1, 0, 2, 'color'], [2, 1, 3, 'a']]
      )
    );
    expect(migrated.nodes[1].type).toBe(SET_MESH_COLOR);
    expect(migrated.edges).toEqual([[1, 0, 2, 'color'], [2, 0, 3, 'a']]);
  });

  it('splits a set_color with paths and a mesh into one of each with the same color', () => {
    const migrated = migrateTo(
      10,
      v5(
        [
          node(1, 'max/myzel/strands'),
          node(2, 'max/myzel/lamellae'),
          node(3, SET_COLOR, { color: pink }),
          node(4, 'max/myzel/hyphae_to_mesh'),
          node(5, 'max/myzel/join_meshes')
        ],
        [[1, 0, 3, 'plant'], [2, 0, 3, 'mesh'], [3, 0, 4, 'paths'], [3, 1, 5, 'b']]
      )
    );
    const mesh = migrated.nodes.find((n) => n.type === SET_MESH_COLOR)!;
    expect(mesh.id).toBe(6);
    expect(mesh.props).toEqual({ color: pink });
    expect(mesh.position).toEqual([30, 0]);
    expect(migrated.nodes.find((n) => n.id === 3)!.type).toBe(SET_COLOR);
    expect(migrated.edges).toEqual([
      [1, 0, 3, 'paths'],
      [2, 0, mesh.id, 'mesh'],
      [3, 0, 4, 'paths'],
      [mesh.id, 0, 5, 'b']
    ]);
  });

  it('gives both nodes a color that comes from a node and the exposed inputs they have', () => {
    const old = node(3, SET_COLOR);
    old.exposed = ['color', 'plant', 'mesh'];
    const migrated = migrateTo(
      10,
      v5(
        [
          node(1, 'max/myzel/mix_color'),
          node(2, 'max/myzel/lamellae'),
          old,
          node(4, 'max/myzel/join_meshes')
        ],
        [[1, 0, 3, 'color'], [2, 0, 3, 'mesh'], [3, 1, 4, 'a'], [3, 0, 4, 'b']]
      )
    );
    const mesh = migrated.nodes.find((n) => n.type === SET_MESH_COLOR)!;
    expect(mesh.exposed).toEqual(['color', 'mesh']);
    expect(migrated.nodes.find((n) => n.id === 3)!.exposed).toEqual(['color', 'paths']);
    expect(migrated.edges).toEqual([
      [1, 0, 3, 'color'],
      [1, 0, mesh.id, 'color'],
      [2, 0, mesh.id, 'mesh'],
      [mesh.id, 0, 4, 'a'],
      [3, 0, 4, 'b']
    ]);
  });

  it('migrates inside groups with ids no other level uses', () => {
    const old = v5([node(9, 'max/myzel/output')]);
    old.groups = [
      {
        id: 0,
        nodes: [node(1, SET_COLOR), node(2, SET_COLOR), node(3, SET_COLOR)],
        edges: [[5, 0, 1, 'plant'], [6, 0, 1, 'mesh'], [1, 1, 7, 'a'], [8, 0, 2, 'mesh']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateTo(10, old);
    const group = migrated.groups![0];
    expect(group.nodes.map((n) => [n.id, n.type])).toEqual([
      [1, SET_COLOR],
      [2, SET_MESH_COLOR],
      [3, SET_COLOR],
      [10, SET_MESH_COLOR]
    ]);
    expect(group.edges).toEqual([
      [5, 0, 1, 'paths'],
      [6, 0, 10, 'mesh'],
      [10, 0, 7, 'a'],
      [8, 0, 2, 'mesh']
    ]);
  });
});

describe('version 7: the Myzel output takes a mesh and no longer meshes paths', () => {
  const v6 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 6
  });
  const OUTPUT = 'max/myzel/output';
  const HYPHAE = 'max/myzel/hyphae_to_mesh';

  it('renames field to mesh and drops the values it lost', () => {
    const migrated = migrateTo(
      10,
      v6(
        [
          node(1, 'max/myzel/join_meshes'),
          node(2, OUTPUT, { show: 1, blend: 5, resolution_field: 96, cut: 1 })
        ],
        [[1, 0, 2, 'field']]
      )
    );
    expect(migrated.nodes.find((n) => n.id === 2)!.props).toEqual({});
    expect(migrated.edges).toEqual([[1, 0, 2, 'mesh']]);
  });

  it('puts hyphae_to_mesh before an output that showed the surface of paths', () => {
    const migrated = migrateTo(
      10,
      v6(
        [node(1, 'max/myzel/strands'), node(2, OUTPUT, { blend: 5 })],
        [[1, 0, 2, 'field']]
      )
    );
    const mesh = migrated.nodes.find((n) => n.type === HYPHAE)!;
    expect(mesh.props).toEqual({ blend: 5 });
    expect(mesh.position).toEqual([0, 0]);
    expect(migrated.nodes.find((n) => n.id === 2)!.position).toEqual([30, 0]);
    expect(migrated.edges).toEqual([[1, 0, mesh.id, 'paths'], [mesh.id, 0, 2, 'mesh']]);
  });

  it('keeps only the last of several edges into field, the one the runtime used', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const migrated = migrateTo(
      10,
      v6(
        [node(1, 'max/myzel/strands'), node(3, 'max/myzel/cap'), node(2, OUTPUT)],
        [[1, 0, 2, 'field'], [3, 0, 2, 'field']]
      )
    );
    const mesh = migrated.nodes.find((n) => n.type === HYPHAE)!;
    expect(migrated.edges).toEqual([[3, 0, mesh.id, 'paths'], [mesh.id, 0, 2, 'mesh']]);
    expect(warn).toHaveBeenCalledOnce();
    warn.mockRestore();
  });

  it('keeps the default blend out of the new node', () => {
    const migrated = migrateTo(
      10,
      v6(
        [node(1, 'max/myzel/set_color'), node(2, OUTPUT, { blend: 3, show: 2 })],
        [[1, 0, 2, 'field']]
      )
    );
    expect(migrated.nodes.find((n) => n.type === HYPHAE)!.props).toEqual({});
  });

  it('leaves paths wired straight into an output that showed them as lines', () => {
    const migrated = migrateTo(
      10,
      v6([node(1, 'max/myzel/strands'), node(2, OUTPUT, { show: 1 })], [[1, 0, 2, 'field']])
    );
    expect(migrated.nodes.map((n) => n.type)).not.toContain(HYPHAE);
    expect(migrated.edges).toEqual([[1, 0, 2, 'mesh']]);
  });

  it('leaves meshes, patterns and the points of path nodes alone', () => {
    for (
      const [type, output] of [
        ['max/myzel/join_meshes', 0],
        ['max/myzel/reaction_diffusion', 0],
        ['max/myzel/cap', 1]
      ] as const
    ) {
      const migrated = migrateTo(
        7,
        v6([node(1, type), node(2, OUTPUT)], [[1, output, 2, 'field']])
      );
      expect(migrated.nodes.map((n) => n.type), type).not.toContain(HYPHAE);
      expect(migrated.edges).toEqual([[1, output, 2, 'mesh']]);
    }
  });

  it('migrates inside groups with ids no other level uses', () => {
    const old = v6([node(9, OUTPUT)]);
    old.groups = [
      {
        id: 0,
        nodes: [node(1, 'max/myzel/strands'), node(2, OUTPUT, { blend: 4 })],
        edges: [[1, 0, 2, 'field']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const group = migrateTo(10, old).groups![0];
    expect(group.nodes.map((n) => [n.id, n.type])).toEqual([
      [1, 'max/myzel/strands'],
      [2, OUTPUT],
      [10, HYPHAE]
    ]);
    expect(group.edges).toEqual([[1, 0, 10, 'paths'], [10, 0, 2, 'mesh']]);
  });
});

describe('version 8: the cap grows at points, the stipe runs along with its rays', () => {
  const v7 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 7
  });
  const CAP = 'max/myzel/cap';
  const MARGIN = 'max/myzel/cap_margin';
  const POINTS = 'max/plantarium/points_on_path';
  const SEPARATE = 'max/plantarium/separate';
  const ofType = (nodes: SerializedNode[], type: string) => nodes.filter((n) => n.type === type);

  it('puts one point at the tip of every stipe in front of the cap', () => {
    const migrated = migrateTo(
      8,
      v7(
        [
          node(1, 'max/plantarium/stem'),
          node(2, CAP, { rays: 14, seed: 5 }),
          node(3, 'max/myzel/margin_waves'),
          node(4, 'max/myzel/cap_open'),
          node(5, 'max/myzel/lamellae'),
          node(6, 'max/myzel/reaction_diffusion'),
          node(7, 'max/plantarium/random')
        ],
        [
          [1, 0, 2, 'plant'],
          [2, 0, 3, 'cap'],
          [2, 1, 3, 'hinge'],
          [3, 0, 4, 'cap'],
          [2, 1, 4, 'hinge'],
          [4, 0, 5, 'cap'],
          [2, 1, 5, 'hinge'],
          [1, 0, 5, 'stipe'],
          [6, 0, 5, 'pattern'],
          [7, 0, 2, 'seed']
        ]
      )
    );
    expect(migrated.version).toBe(8);
    const [points] = ofType(migrated.nodes, POINTS);
    expect(points.props).toEqual({ amount: 1, lowest_point: 1, highest_point: 1 });
    expect(migrated.nodes.find((n) => n.id === 2)!.props).toEqual({ rays: 14 });
    expect(ofType(migrated.nodes, MARGIN)).toEqual([]);
    expect(ofType(migrated.nodes, SEPARATE)).toEqual([]);
    expect(migrated.edges).toEqual([
      [1, 0, 2, 'plant'],
      [2, 0, 3, 'cap'],
      [3, 0, 4, 'cap'],
      [4, 0, 5, 'cap'],
      [6, 0, 5, 'pattern'],
      [1, 0, points.id, 'plant'],
      [points.id, 0, 2, 'points']
    ]);
  });

  it('reads the hinge from the points and the margin from a cap_margin', () => {
    const migrated = migrateTo(
      10,
      v7(
        [
          node(1, 'max/plantarium/stem'),
          node(2, CAP),
          node(3, 'max/myzel/margin_curl'),
          node(4, 'max/myzel/cap_open'),
          node(5, 'max/myzel/membrane'),
          node(6, 'max/myzel/membrane'),
          node(7, 'max/plantarium/instance_from_points')
        ],
        [
          [1, 0, 2, 'plant'],
          [2, 0, 3, 'cap'],
          [2, 1, 3, 'hinge'],
          [3, 0, 4, 'cap'],
          [4, 1, 5, 'a'],
          [4, 1, 6, 'a'],
          [2, 2, 6, 'b'],
          [2, 1, 7, 'points']
        ]
      )
    );
    const [points] = ofType(migrated.nodes, POINTS);
    const [fromCap, fromOpen] = ofType(migrated.nodes, MARGIN);
    expect(migrated.edges).toEqual([
      [1, 0, 2, 'plant'],
      [2, 0, 3, 'cap'],
      [3, 0, 4, 'cap'],
      [fromOpen.id, 0, 5, 'a'],
      [fromOpen.id, 0, 6, 'a'],
      [fromCap.id, 0, 6, 'b'],
      [points.id, 0, 7, 'points'],
      [1, 0, points.id, 'plant'],
      [points.id, 0, 2, 'points'],
      [2, 0, fromCap.id, 'cap'],
      [4, 0, fromOpen.id, 'cap']
    ]);
  });

  it('leaves the stipe behind where the rays go on to other nodes', () => {
    const migrated = migrateTo(
      10,
      v7(
        [
          node(1, 'max/plantarium/stem'),
          node(2, CAP),
          node(3, 'max/plantarium/fill_gaps'),
          node(4, 'max/myzel/cap_open'),
          node(5, 'max/myzel/strands'),
          node(6, 'max/myzel/hyphae_to_mesh'),
          node(7, 'max/myzel/lamellae')
        ],
        [
          [1, 0, 2, 'plant'],
          [2, 0, 3, 'plant'],
          [3, 0, 4, 'cap'],
          [4, 0, 5, 'plant'],
          [4, 0, 6, 'paths'],
          [4, 0, 7, 'cap']
        ]
      )
    );
    const [separate] = ofType(migrated.nodes, SEPARATE);
    const [points] = ofType(migrated.nodes, POINTS);
    expect(migrated.edges).toEqual([
      [1, 0, 2, 'plant'],
      [2, 0, 3, 'plant'],
      [3, 0, 4, 'cap'],
      [separate.id, 1, 5, 'plant'],
      [separate.id, 1, 6, 'paths'],
      [4, 0, 7, 'cap'],
      [1, 0, points.id, 'plant'],
      [points.id, 0, 2, 'points'],
      [4, 0, separate.id, 'plant'],
      [1, 0, separate.id, 'like']
    ]);
  });

  it('drops an exposed hinge and gives an unwired cap no points', () => {
    const lamellae = node(3, 'max/myzel/lamellae');
    lamellae.exposed = ['hinge', 'stipe', 'depth'];
    const migrated = migrateTo(
      10,
      v7(
        [node(2, CAP), node(4, 'max/plantarium/instance_from_points'), lamellae],
        [[2, 1, 4, 'points'], [2, 1, 3, 'hinge'], [2, 0, 3, 'cap']]
      )
    );
    expect(migrated.nodes.find((n) => n.id === 3)!.exposed).toEqual(['depth']);
    expect(ofType(migrated.nodes, POINTS)).toEqual([]);
    expect(migrated.edges).toEqual([[2, 0, 3, 'cap']]);
  });

  it('migrates inside groups and removes the group sockets that carried the hinge', () => {
    const old = v7([
      node(1, 'max/plantarium/stem'),
      { ...node(2, '__internal/group/instance', { groupId: 20 }) },
      { ...node(3, '__internal/group/instance', { groupId: 21, hinge: 0, depth: 0.004 }) }
    ], [[1, 0, 2, 'stipe'], [2, 0, 3, 'rays'], [2, 1, 3, 'hinge'], [2, 2, 3, 'depth']]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, CAP),
          node(13, 'max/myzel/cap_open')
        ],
        edges: [
          [10, 0, 12, 'plant'],
          [12, 0, 13, 'cap'],
          [12, 1, 13, 'hinge'],
          [13, 0, 11, 'out_0'],
          [12, 1, 11, 'out_1'],
          [13, 1, 11, 'out_2']
        ],
        inputs: { stipe: { type: 'path', external: true } },
        outputs: [{ type: 'path' }, { type: 'points', label: 'Hinge' }, { type: 'points' }]
      },
      {
        id: 21,
        nodes: [
          node(14, '__internal/group/input'),
          node(15, 'max/myzel/lamellae'),
          node(16, 'max/myzel/membrane')
        ],
        edges: [[14, 0, 15, 'cap'], [14, 1, 15, 'hinge'], [14, 2, 15, 'depth'], [14, 2, 16, 'a']],
        inputs: {
          rays: { type: 'path', external: true },
          hinge: { type: 'points', external: true },
          depth: { type: 'points', external: true }
        }
      }
    ] as unknown as NonNullable<Graph['groups']>;
    const migrated = migrateTo(10, old);
    const [cap, gills] = migrated.groups!;
    const [points] = ofType(cap.nodes, POINTS);
    const [margin] = ofType(cap.nodes, MARGIN);
    const ids = [
      ...migrated.nodes.map((n) => n.id),
      ...migrated.groups!.flatMap((g) => [g.id, ...g.nodes.map((n) => n.id)])
    ];
    expect(new Set(ids).size).toBe(ids.length);
    expect(cap.outputs).toEqual([{ type: 'path' }, { type: 'points' }]);
    expect(cap.edges).toEqual([
      [10, 0, 12, 'plant'],
      [12, 0, 13, 'cap'],
      [13, 0, 11, 'out_0'],
      [margin.id, 0, 11, 'out_1'],
      [10, 0, points.id, 'plant'],
      [points.id, 0, 12, 'points'],
      [13, 0, margin.id, 'cap']
    ]);
    expect(Object.keys(gills.inputs!)).toEqual(['rays', 'depth']);
    expect(gills.edges).toEqual([[14, 0, 15, 'cap'], [14, 1, 15, 'depth'], [14, 1, 16, 'a']]);
    expect(migrated.nodes.find((n) => n.id === 3)!.props).toEqual({ groupId: 21, depth: 0.004 });
    expect(migrated.edges).toEqual([[1, 0, 2, 'stipe'], [2, 0, 3, 'rays'], [2, 1, 3, 'depth']]);
  });
});

describe('version 9: a curve into the cap radius makes it lopsided, cap_lopsided is gone', () => {
  const v8 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 8
  });
  const CAP = 'max/myzel/cap';
  const LOPSIDED = 'max/myzel/cap_lopsided';
  const RANDOM = 'max/plantarium/random';

  it('links the neighbours of cap_lopsided, the cap becomes round', () => {
    const migrated = migrateTo(
      9,
      v8(
        [
          node(1, CAP),
          node(2, 'max/myzel/margin_waves'),
          node(3, LOPSIDED, { amount: 0.6, offset: 1 }),
          node(4, 'max/myzel/margin_curl'),
          node(5, 'max/plantarium/separate'),
          node(6, RANDOM, { min: -10, max: 10 })
        ],
        [
          [1, 0, 2, 'cap'],
          [2, 0, 3, 'cap'],
          [6, 0, 3, 'tilt'],
          [3, 0, 4, 'cap'],
          [3, 0, 5, 'plant']
        ]
      )
    );
    expect(migrated.version).toBe(9);
    expect(migrated.nodes.map((n) => n.type)).not.toContain(LOPSIDED);
    expect(migrated.nodes.find((n) => n.id === 6)!.props).toEqual({ min: -10, max: 10 });
    expect(migrated.edges).toEqual([[1, 0, 2, 'cap'], [2, 0, 4, 'cap'], [2, 0, 5, 'plant']]);
  });

  it('drops the edges out of a cap_lopsided that got nothing', () => {
    const migrated = migrateGraph(
      v8([node(3, LOPSIDED), node(4, 'max/myzel/margin_curl')], [[3, 0, 4, 'cap']])
    );
    expect(migrated.nodes.map((n) => n.id)).toEqual([4]);
    expect(migrated.edges).toEqual([]);
  });

  it('makes the random nodes into the cap radius draw once per path', () => {
    const migrated = migrateGraph(
      v8(
        [
          node(1, CAP),
          node(2, 'max/plantarium/math', { op_type: 2 }),
          node(3, RANDOM, { min: 0.9, max: 1.1 }),
          node(4, 'max/plantarium/curve'),
          node(5, RANDOM, { min: 0, max: 1, uniform: 1 }),
          node(6, RANDOM, { min: 0.001, max: 0.002 })
        ],
        [
          [2, 0, 1, 'radius'],
          [3, 0, 2, 'a'],
          [4, 0, 2, 'b'],
          [5, 0, 4, 'value'],
          [6, 0, 1, 'thickness']
        ]
      )
    );
    const props = (id: number) => migrated.nodes.find((n) => n.id === id)!.props;
    expect(props(3)).toEqual({ min: 0.9, max: 1.1, per_path: 1 });
    expect(props(5)).toEqual({ min: 0, max: 1, uniform: 1, per_path: 1 });
    expect(props(6)).toEqual({ min: 0.001, max: 0.002 });
  });

  it('migrates inside groups', () => {
    const old = v8([node(1, '__internal/group/instance', { groupId: 20 })]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, CAP),
          node(13, LOPSIDED)
        ],
        edges: [[10, 0, 12, 'plant'], [12, 0, 13, 'cap'], [13, 0, 11, 'out_0']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const [group] = migrateGraph(old).groups!;
    expect(group.nodes.map((n) => n.id)).toEqual([10, 11, 12]);
    expect(group.edges).toEqual([[10, 0, 12, 'plant'], [12, 0, 11, 'out_0']]);
  });
});

describe('version 10: strands only fills the tube, twist and random_break come behind it', () => {
  const v9 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 9
  });
  const STRANDS = 'max/myzel/strands';
  const TWIST = 'max/plantarium/twist';
  const BREAK = 'max/plantarium/random_break';
  const byType = (g: { nodes: SerializedNode[] }, type: string) =>
    g.nodes.filter((n) => n.type === type);

  it('only drops the inputs at their defaults', () => {
    const migrated = migrateGraph(
      v9([node(1, STRANDS, { amount: 12, twist: 0, length_variation: 0 })])
    );
    expect(migrated.nodes.map((n) => n.type)).toEqual([STRANDS]);
    expect(migrated.nodes[0].props).toEqual({ amount: 12 });
  });

  it('turns a twist into a twist node behind strands', () => {
    const migrated = migrateGraph(
      v9(
        [node(1, STRANDS, { twist: 0.1, wobble: 0.25 }), node(2, 'max/plantarium/clump')],
        [[1, 0, 2, 'plant']]
      )
    );
    const [twist] = byType(migrated, TWIST);
    expect(twist.props).toEqual({ turns: 0.1 });
    expect(migrated.nodes.find((n) => n.id === 1)!.props).toEqual({ wobble: 0.25 });
    expect(byType(migrated, BREAK)).toEqual([]);
    expect(migrated.edges).toEqual([[twist.id, 0, 2, 'plant'], [1, 0, twist.id, 'plant']]);
  });

  it('turns a length variation into a random_break that keeps the mean shortening', () => {
    const migrated = migrateGraph(
      v9(
        [node(1, STRANDS, { length_variation: 0.3 }), node(2, 'max/plantarium/clump')],
        [[1, 0, 2, 'plant']]
      )
    );
    const [randomBreak] = byType(migrated, BREAK);
    expect(randomBreak.props).toEqual({ chance: 0.5, lowest_break: 0.85, highest_break: 1 });
    expect(byType(migrated, TWIST)).toEqual([]);
    expect(migrated.nodes.find((n) => n.id === 1)!.props).toEqual({});
    expect(migrated.edges).toEqual([
      [randomBreak.id, 0, 2, 'plant'],
      [1, 0, randomBreak.id, 'plant']
    ]);
  });

  it('twists before it breaks, and passes the other inputs on to strands', () => {
    const migrated = migrateGraph(
      v9(
        [
          node(1, 'max/plantarium/stem'),
          node(2, 'max/plantarium/float'),
          node(3, STRANDS, { twist: 0.05, length_variation: 0.15 }),
          node(4, 'max/plantarium/clump')
        ],
        [[1, 0, 3, 'plant'], [2, 0, 3, 'thickness'], [3, 0, 4, 'plant']]
      )
    );
    const [twist] = byType(migrated, TWIST);
    const [randomBreak] = byType(migrated, BREAK);
    expect(randomBreak.props).toMatchObject({ lowest_break: 0.925 });
    expect(migrated.edges).toHaveLength(5);
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, 3, 'plant'],
        [2, 0, 3, 'thickness'],
        [3, 0, twist.id, 'plant'],
        [twist.id, 0, randomBreak.id, 'plant'],
        [randomBreak.id, 0, 4, 'plant']
      ])
    );
  });

  it('wires a wired twist into the twist node', () => {
    const migrated = migrateGraph(
      v9([node(1, 'max/plantarium/float'), node(2, STRANDS)], [[1, 0, 2, 'twist']])
    );
    const [twist] = byType(migrated, TWIST);
    expect(migrated.edges).toEqual([[1, 0, twist.id, 'turns'], [2, 0, twist.id, 'plant']]);
  });

  it('builds 1 - length_variation / 2 from math nodes for a wired length variation', () => {
    const migrated = migrateGraph(
      v9([node(1, 'max/plantarium/float'), node(2, STRANDS)], [[1, 0, 2, 'length_variation']])
    );
    const [randomBreak] = byType(migrated, BREAK);
    const [half, lowest] = byType(migrated, 'max/plantarium/math');
    expect(half.props).toMatchObject({ op_type: 2, b: 0.5 });
    expect(lowest.props).toMatchObject({ op_type: 1, a: 1 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, half.id, 'a'],
        [half.id, 0, lowest.id, 'b'],
        [lowest.id, 0, randomBreak.id, 'lowest_break'],
        [2, 0, randomBreak.id, 'plant']
      ])
    );
    expect(migrated.edges.some((e) => e[3] === 'length_variation')).toBe(false);
  });

  it('migrates inside groups, and the new nodes do not share ids with other levels', () => {
    const old = v9([node(1, '__internal/group/instance', { groupId: 20 })]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, STRANDS, { twist: 0.1, length_variation: 0.1 })
        ],
        edges: [[10, 0, 12, 'plant'], [12, 0, 11, 'out_0']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateGraph(old);
    const [group] = migrated.groups!;
    const [twist] = byType(group, TWIST);
    const [randomBreak] = byType(group, BREAK);
    const ids = [1, group.id, ...group.nodes.map((n) => n.id)];
    expect(new Set(ids).size).toBe(ids.length);
    expect(group.edges).toEqual(
      expect.arrayContaining([
        [10, 0, 12, 'plant'],
        [12, 0, twist.id, 'plant'],
        [twist.id, 0, randomBreak.id, 'plant'],
        [randomBreak.id, 0, 11, 'out_0']
      ])
    );
  });
});

describe('version 11: a curve node shapes the depth of lamellae, ridges makes the blunt folds', () => {
  const v10 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 10
  });
  const LAMELLAE = 'max/myzel/lamellae';
  const RIDGES = 'max/myzel/ridges';
  const CURVE = 'max/plantarium/curve';
  const OLD_CURVE = [0, 0.3, 0.2, 0.85, 0.45, 1, 0.8, 0.65, 1, 0.1];
  const byType = (g: { nodes: SerializedNode[] }, type: string) =>
    g.nodes.filter((n) => n.type === type);

  it('draws the depth curve with a curve node up to the old depth', () => {
    const migrated = migrateGraph(
      v10(
        [
          node(1, 'max/myzel/cap_open'),
          node(2, LAMELLAE, {
            depth: 0.0035,
            depth_curve: [0, 1, 1, 0.12],
            free_distance: 0,
            decurrent: 0.014,
            blunt: 0
          }),
          node(3, 'max/myzel/join_meshes')
        ],
        [[1, 0, 2, 'cap'], [2, 0, 3, 'b']]
      )
    );
    const [lamellae] = byType(migrated, LAMELLAE);
    const [curve] = byType(migrated, CURVE);
    expect(lamellae.props).toEqual({ free_distance: 0, decurrent: 0.014 });
    expect(curve.props).toEqual({ curve: [0, 1, 1, 0.12], min: 0, max: 0.0035 });
    expect(curve.exposed).toBeUndefined();
    expect(migrated.edges).toEqual(
      expect.arrayContaining([[1, 0, 2, 'cap'], [2, 0, 3, 'b'], [curve.id, 0, 2, 'depth']])
    );
    expect(migrated.edges).toHaveLength(3);
  });

  it('draws the old default curve and depth when none were saved', () => {
    const migrated = migrateGraph(v10([node(1, LAMELLAE)]));
    const [curve] = byType(migrated, CURVE);
    expect(curve.props).toEqual({ curve: OLD_CURVE, min: 0, max: 0.004 });
    expect(byType(migrated, LAMELLAE)[0].props).toEqual({});
  });

  it('multiplies a depth that comes from a node by the curve through its max', () => {
    const migrated = migrateGraph(
      v10(
        [node(1, CURVE, { curve: [0, 0, 1, 1], min: 0.0005 }), node(2, LAMELLAE, { depth: 0.002 })],
        [[1, 0, 2, 'depth']]
      )
    );
    const added = byType(migrated, CURVE).find((n) => n.id !== 1)!;
    expect(added.props).toEqual({ curve: OLD_CURVE, min: 0 });
    expect(added.exposed).toEqual(['max']);
    expect(migrated.edges).toEqual([[1, 0, added.id, 'max'], [added.id, 0, 2, 'depth']]);
    expect(byType(migrated, LAMELLAE)[0].props).toEqual({});
  });

  it('adds no curve for a depth curve that is 1 all along', () => {
    const migrated = migrateGraph(
      v10(
        [node(1, 'max/plantarium/float'), node(2, LAMELLAE, { depth_curve: [0, 1, 1, 1] })],
        [[1, 0, 2, 'depth']]
      )
    );
    expect(byType(migrated, CURVE)).toEqual([]);
    expect(migrated.edges).toEqual([[1, 0, 2, 'depth']]);
  });

  it('turns a blunt lamellae into ridges with the values it had', () => {
    const migrated = migrateGraph(
      v10(
        [
          node(1, LAMELLAE, { depth: 0.0012, free_distance: 0, blunt: 1, edge_wave: 0.4 }),
          node(2, 'max/plantarium/float')
        ],
        [[2, 0, 1, 'free_distance']]
      )
    );
    expect(byType(migrated, LAMELLAE)).toEqual([]);
    const [ridges] = byType(migrated, RIDGES);
    expect(ridges.id).toBe(1);
    expect(ridges.props).toEqual({ decurrent: 0, thickness: 0.0003, edge_wave: 0.4 });
    const [curve] = byType(migrated, CURVE);
    expect(curve.props).toMatchObject({ max: 0.0012 });
    expect(migrated.edges).toEqual([[curve.id, 0, 1, 'depth']]);
  });

  it('keeps thin and half blunt gills as lamellae', () => {
    const migrated = migrateGraph(
      v10([node(1, LAMELLAE, { blunt: 0.5, free_distance: 0.002 })])
    );
    const [lamellae] = byType(migrated, LAMELLAE);
    expect(lamellae.props).toEqual({ free_distance: 0.002 });
    expect(byType(migrated, RIDGES)).toEqual([]);
  });

  it('drops the exposed inputs that are gone', () => {
    const old = node(1, LAMELLAE, { blunt: 1 });
    old.exposed = ['depth', 'depth_curve', 'blunt', 'free_distance', 'thickness'];
    const [ridges] = byType(migrateGraph(v10([old])), RIDGES);
    expect(ridges.exposed).toEqual(['depth', 'thickness']);
  });

  it('migrates inside groups, and the new nodes do not share ids with other levels', () => {
    const old = v10([node(1, '__internal/group/instance', { groupId: 20 })]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, LAMELLAE, { thickness: 0.0003 })
        ],
        edges: [[10, 0, 12, 'cap'], [10, 1, 12, 'depth'], [12, 0, 11, 'out_0']]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateGraph(old);
    const [group] = migrated.groups!;
    const [curve] = byType(group, CURVE);
    const ids = [1, group.id, ...group.nodes.map((n) => n.id)];
    expect(new Set(ids).size).toBe(ids.length);
    expect(group.edges).toEqual(
      expect.arrayContaining([
        [10, 0, 12, 'cap'],
        [10, 1, curve.id, 'max'],
        [curve.id, 0, 12, 'depth'],
        [12, 0, 11, 'out_0']
      ])
    );
    expect(group.edges).toHaveLength(4);
  });
});

describe('version 12: a curve shapes the curl, a random the height of every lobe', () => {
  const v11 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 11
  });
  const CURL = 'max/myzel/margin_curl';
  const WAVES = 'max/myzel/margin_waves';
  const CURVE = 'max/plantarium/curve';
  const RANDOM = 'max/plantarium/random';
  const MATH = 'max/plantarium/math';
  const PLAIN_WAVES = [0, 0, 0.5, 0.25, 1, 1];
  const byType = (g: { nodes: SerializedNode[] }, type: string) =>
    g.nodes.filter((n) => n.type === type);
  const smoothstep = (t: number) => t * t * (3 - 2 * t);

  it('only drops reach where a plain amount spreads as before', () => {
    const migrated = migrateGraph(
      v11([node(1, CURL, { amount: 0.12, reach: 0.35 }), node(2, CURL)])
    );
    expect(migrated.nodes.map((n) => n.props)).toEqual([{ amount: 0.12 }, {}]);
  });

  it('draws another reach with a curve up to the amount', () => {
    const migrated = migrateGraph(v11([node(1, CURL, { amount: 0.4, reach: 0.5 })]));
    const [curl] = byType(migrated, CURL);
    const [curve] = byType(migrated, CURVE);
    expect(curl.props).toEqual({});
    expect(curve.props).toMatchObject({ min: 0, max: 0.4 });
    const points = curve.props!.curve as number[];
    expect(points.slice(0, 4)).toEqual([0, 0, 0.5, 0]);
    for (let i = 2; i < points.length; i += 2) {
      expect(points[i + 1]).toBeCloseTo(smoothstep((points[i] - 0.5) / 0.5), 5);
    }
    expect(migrated.edges).toEqual([[curve.id, 0, 1, 'amount']]);
  });

  it('puts an amount from a node into the max of the curve', () => {
    const migrated = migrateGraph(
      v11(
        [node(1, CURVE, { min: 0.1, max: 0.5 }), node(2, CURL), node(3, 'max/plantarium/float')],
        [[1, 0, 2, 'amount'], [3, 0, 2, 'reach']]
      )
    );
    const added = byType(migrated, CURVE).find((n) => n.id !== 1)!;
    expect(added.props).toMatchObject({ min: 0 });
    expect(added.props!.max).toBeUndefined();
    expect(added.exposed).toEqual(['max']);
    expect((added.props!.curve as number[]).slice(0, 4)).toEqual([0, 0, 0.65, 0]);
    // a reach from a node can't be drawn, it goes
    expect(migrated.edges).toEqual([[1, 0, added.id, 'max'], [added.id, 0, 2, 'amount']]);
  });

  it('turns a plain irregularity into uneven spacing and a random amplitude', () => {
    const migrated = migrateGraph(
      v11([node(1, WAVES, { waves: 5, amplitude: 0.18, irregularity: 0.8, seed: 3 })])
    );
    const [waves] = byType(migrated, WAVES);
    const [curve] = byType(migrated, CURVE);
    const [random] = byType(migrated, RANDOM);
    expect(waves.props).toEqual({ waves: 5, uneven_spacing: 0.8 });
    expect(curve.props).toEqual({ curve: PLAIN_WAVES, min: 0 });
    expect(random.props!.min).toBeCloseTo(0.108);
    expect(random.props!.max).toBeCloseTo(0.252);
    expect(random.props!.per_path).toBeUndefined();
    expect(migrated.edges).toEqual(
      expect.arrayContaining([[random.id, 0, curve.id, 'max'], [curve.id, 0, 1, 'amplitude']])
    );
    expect(migrated.edges).toHaveLength(2);
  });

  it('gives the old default irregularity a random and leaves out an irregularity of 0', () => {
    const migrated = migrateGraph(v11([node(1, WAVES), node(2, WAVES, { irregularity: 0 })]));
    const [random] = byType(migrated, RANDOM);
    expect(random.props!.min).toBeCloseTo(0.1275);
    expect(random.props!.max).toBeCloseTo(0.1725);
    expect(byType(migrated, WAVES).map((n) => n.props)).toEqual([{}, { uneven_spacing: 0 }]);
    expect(byType(migrated, CURVE)).toHaveLength(1);
  });

  it('multiplies an amplitude from a node by a random share, its random nodes draw once per cap', () => {
    const migrated = migrateGraph(
      v11(
        [
          node(1, RANDOM, { min: 0.75, max: 1.15 }),
          node(2, CURVE, { curve: [0, 0, 1, 1] }),
          node(3, WAVES, { irregularity: 0.5 })
        ],
        [[1, 0, 2, 'max'], [2, 0, 3, 'amplitude']]
      )
    );
    const old = migrated.nodes.find((n) => n.id === 1)!;
    expect(old.props).toEqual({ min: 0.75, max: 1.15, per_path: 1 });
    const share = byType(migrated, RANDOM).find((n) => n.id !== 1)!;
    expect(share.props).toEqual({ min: 0.75, max: 1.25 });
    const [product] = byType(migrated, MATH);
    expect(product.props).toEqual({ op_type: 2 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, 2, 'max'],
        [2, 0, product.id, 'a'],
        [share.id, 0, product.id, 'b'],
        [product.id, 0, 3, 'amplitude']
      ])
    );
    expect(migrated.edges).toHaveLength(4);
  });

  it('builds 1 + irregularity × random(-0.5, 0.5) for an irregularity from a node', () => {
    const migrated = migrateGraph(
      v11(
        [node(1, RANDOM, { min: 0.5, max: 1 }), node(2, WAVES, { amplitude: 0.1 })],
        [[1, 0, 2, 'irregularity']]
      )
    );
    const random = byType(migrated, RANDOM).find((n) => n.id !== 1)!;
    expect(random.props).toEqual({ min: -0.5, max: 0.5 });
    const [spread, share, scaled] = byType(migrated, MATH);
    expect(spread.props).toEqual({ op_type: 2 });
    expect(share.props).toEqual({ op_type: 0, b: 1 });
    expect(scaled.props).toEqual({ op_type: 2, a: 0.1 });
    const [curve] = byType(migrated, CURVE);
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, 2, 'uneven_spacing'],
        [1, 0, spread.id, 'b'],
        [random.id, 0, spread.id, 'a'],
        [spread.id, 0, share.id, 'a'],
        [share.id, 0, scaled.id, 'b'],
        [scaled.id, 0, curve.id, 'max'],
        [curve.id, 0, 2, 'amplitude']
      ])
    );
    expect(migrated.edges).toHaveLength(7);
    // drawn for every cap, it goes on into the uneven spacing too
    expect(migrated.nodes.find((n) => n.id === 1)!.props).toEqual({ min: 0.5, max: 1 });
  });

  it('renames an exposed irregularity and drops exposed inputs that are gone', () => {
    const waves = node(1, WAVES, { irregularity: 0 });
    waves.exposed = ['irregularity', 'seed', 'waves'];
    const curl = node(2, CURL);
    curl.exposed = ['amount', 'reach'];
    const migrated = migrateGraph(v11([waves, curl]));
    expect(byType(migrated, WAVES)[0].exposed).toEqual(['uneven_spacing', 'waves']);
    expect(byType(migrated, CURL)[0].exposed).toEqual(['amount']);
  });

  it('migrates inside groups, and the new nodes do not share ids with other levels', () => {
    const old = v11([node(1, '__internal/group/instance', { groupId: 20 })]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, CURL),
          node(13, WAVES, { irregularity: 0.4 })
        ],
        edges: [[10, 0, 13, 'cap'], [10, 1, 12, 'amount'], [13, 0, 12, 'cap'], [
          12,
          0,
          11,
          'out_0'
        ]]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateGraph(old);
    const [group] = migrated.groups!;
    const ids = [1, group.id, ...group.nodes.map((n) => n.id)];
    expect(new Set(ids).size).toBe(ids.length);
    const amount = byType(group, CURVE).find((n) => n.exposed?.includes('max'))!;
    expect(group.edges).toEqual(
      expect.arrayContaining([
        [10, 0, 13, 'cap'],
        [10, 1, amount.id, 'max'],
        [amount.id, 0, 12, 'amount'],
        [13, 0, 12, 'cap'],
        [12, 0, 11, 'out_0']
      ])
    );
    expect(group.edges).toHaveLength(7);
  });
});

describe('version 13: membrane wraps around a point, skirts hang the ring and the fringe', () => {
  const v12 = (nodes: SerializedNode[], edges: Graph['edges'] = []): Graph => ({
    ...graph(nodes, edges),
    version: 12
  });
  const MEMBRANE = 'max/myzel/membrane';
  const SKIRT = 'max/myzel/skirt';
  const POINTS = 'max/plantarium/points_on_path';
  const CURVE = 'max/plantarium/curve';
  const MATH = 'max/plantarium/math';
  const JOIN = 'max/myzel/join_meshes';
  const OLD_FLARE = [0, 0.45, 0.4, 0.25, 1, 0.12];
  const byType = (g: { nodes: SerializedNode[] }, type: string) =>
    g.nodes.filter((n) => n.type === type);
  // a stipe into b, a margin into a, the membrane into an output
  const veil = (props: SerializedNode['props'], edges: Graph['edges'] = []) =>
    v12(
      [
        node(1, 'max/plantarium/stem'),
        node(2, 'max/myzel/cap_margin'),
        node(3, MEMBRANE, props),
        node(4, 'max/myzel/output'),
        node(5, CURVE)
      ],
      [[1, 0, 3, 'b'], [2, 0, 3, 'a'], [3, 0, 4, 'mesh'], ...edges]
    );

  it('places the membrane with a points_on_path at along and drops the other inputs', () => {
    const migrated = migrateGraph(
      veil({ along: 0.7, attachment: 0.5, thickness: 0.002, sag: 0.1, flare: [0, 1, 1, 1] })
    );
    const [membrane] = byType(migrated, MEMBRANE);
    const [points] = byType(migrated, POINTS);
    expect(membrane.props).toEqual({ thickness: 0.002, sag: 0.1 });
    expect(points.props).toEqual({ amount: 1, lowest_point: 0.7, highest_point: 0.7 });
    expect(byType(migrated, SKIRT)).toHaveLength(0);
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, points.id, 'plant'],
        [points.id, 0, 3, 'b'],
        [2, 0, 3, 'a'],
        [3, 0, 4, 'mesh']
      ])
    );
    expect(migrated.edges).toHaveLength(4);
  });

  it('wires an along from a node into the lowest and highest point', () => {
    const migrated = migrateGraph(veil({}, [[5, 0, 3, 'along']]));
    const [points] = byType(migrated, POINTS);
    expect(points.props).toEqual({ amount: 1 });
    expect(points.exposed).toEqual(['lowest_point', 'highest_point']);
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [5, 0, points.id, 'lowest_point'],
        [5, 0, points.id, 'highest_point']
      ])
    );
  });

  it('holds an anchor by dividing it by the length of the path', () => {
    const migrated = migrateGraph(veil({ anchor: 0.025 }));
    const [points] = byType(migrated, POINTS);
    const [share] = byType(migrated, MATH);
    const [length] = byType(migrated, 'max/plantarium/path_info');
    expect(share.props).toEqual({ op_type: 3, a: 0.025 });
    expect(length.props).toEqual({ info: 5 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [length.id, 0, share.id, 'b'],
        [share.id, 0, points.id, 'lowest_point'],
        [share.id, 0, points.id, 'highest_point'],
        [points.id, 0, 3, 'b']
      ])
    );
  });

  it('leaves a membrane without along or anchor on its path, level with a', () => {
    const migrated = migrateGraph(veil({ attachment: -0.5, sag: 0.2 }));
    expect(byType(migrated, POINTS)).toHaveLength(0);
    expect(byType(migrated, MEMBRANE)[0].props).toEqual({ sag: 0.2 });
    expect(migrated.edges).toEqual([[1, 0, 3, 'b'], [2, 0, 3, 'a'], [3, 0, 4, 'mesh']]);
  });

  it('hangs a ring and a fringe from a tear that comes from a node, the membrane goes at the switch', () => {
    const migrated = migrateGraph(
      veil(
        { along: 0.75, thickness: 0.0006, tear_position: 0.9, torn_length: 0.011, seed: 4 },
        [[5, 0, 3, 'tear']]
      )
    );
    const [membrane] = byType(migrated, MEMBRANE);
    const [points] = byType(migrated, POINTS);
    const [ring, fringe] = byType(migrated, SKIRT);
    const [join] = byType(migrated, JOIN);
    const curves = byType(migrated, CURVE).filter((n) => n.id !== 5);
    const into = (id: number, input: string) =>
      curves.find((c) =>
        migrated.edges.some((e) => e[0] === c.id && e[2] === id && e[3] === input)
      )!;
    expect(membrane.props).toEqual({});
    expect(ring.props).toEqual({ thickness: 0.0006, seed: 4 });
    // the old parts less the recoil, they grow in at the switch
    expect(into(ring.id, 'length').props).toEqual({
      curve: [0, 0, 0.25, 0, 0.26, 1, 1, 1],
      min: 0,
      max: 0.00693
    });
    expect(into(fringe.id, 'length').props!.max).toBeCloseTo(0.00077);
    expect(into(3, 'thickness').props).toEqual({
      curve: [0, 1, 0.25, 1, 0.26, 0, 1, 0],
      min: 0,
      max: 0.0006
    });
    const flare = into(ring.id, 'flare');
    expect(flare.props).toEqual({ curve: OLD_FLARE, min: 0, max: 1 });
    expect(flare.exposed).toEqual(['min']);
    expect(into(flare.id, 'min').props).toEqual({ curve: [0, 0.55, 0.25, 0.55, 0.8, 0, 1, 0] });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [points.id, 0, 3, 'b'],
        [points.id, 0, ring.id, 'attachment'],
        [2, 0, fringe.id, 'attachment'],
        [flare.id, 0, fringe.id, 'flare'],
        [3, 0, join.id, 'a'],
        [ring.id, 0, join.id, 'b'],
        [fringe.id, 0, join.id, 'c'],
        [join.id, 0, 4, 'mesh']
      ])
    );
    // the tear drives the switches and the fall
    expect(migrated.edges.filter((e) => e[0] === 5)).toHaveLength(4);
  });

  it('gives a membrane torn all round only its skirts, hanging at their flare', () => {
    const migrated = migrateGraph(veil({ along: 0.75, tear: 1, tear_position: 0.5 }));
    expect(byType(migrated, MEMBRANE)).toHaveLength(0);
    const skirts = byType(migrated, SKIRT);
    // as long as a span of 1 cm made them, without a torn length
    expect(skirts.map((s) => s.props)).toEqual([
      { length: 0.0035, thickness: 0.001 },
      { length: 0.0035, thickness: 0.001 }
    ]);
    const [flare] = byType(migrated, CURVE).filter((n) => n.id !== 5);
    expect(flare.props).toEqual({ curve: OLD_FLARE, min: 0, max: 1 });
    const [join] = byType(migrated, JOIN);
    expect(migrated.edges).toEqual(
      expect.arrayContaining([[2, 0, skirts[1].id, 'attachment'], [join.id, 0, 4, 'mesh']])
    );
    expect(migrated.edges.some((e) => e[0] === 2 && e[2] === 3)).toBe(false);
  });

  it('leaves out a fringe the tear left too short, and places the ring when the membrane followed a', () => {
    const migrated = migrateGraph(veil({ tear: 0.5, tear_position: 0.99 }));
    const skirts = byType(migrated, SKIRT);
    expect(skirts).toHaveLength(1);
    const [points] = byType(migrated, POINTS);
    expect(points.props).toEqual({ amount: 1, lowest_point: 0.75, highest_point: 0.75 });
    expect(migrated.edges).toEqual(
      expect.arrayContaining([
        [1, 0, points.id, 'plant'],
        [points.id, 0, skirts[0].id, 'attachment']
      ])
    );
    // half way to fallen
    const [flare] = byType(migrated, CURVE).filter((n) => n.id !== 5);
    expect(flare.props!.min).toBeCloseTo(0.55 * (1 - 0.25 / 0.55), 5);
  });

  it('drops the exposed inputs that are gone', () => {
    const membrane = node(1, MEMBRANE, { torn_length: 0.01 });
    membrane.exposed = ['torn_length', 'sag'];
    const migrated = migrateGraph(v12([membrane]));
    expect(byType(migrated, MEMBRANE)[0].exposed).toEqual(['sag']);
  });

  it('migrates inside groups, and the new nodes do not share ids with other levels', () => {
    const old = v12([node(1, '__internal/group/instance', { groupId: 20 })]);
    old.groups = [
      {
        id: 20,
        nodes: [
          node(10, '__internal/group/input'),
          node(11, '__internal/group/output'),
          node(12, MEMBRANE, { tear_position: 0.9, torn_length: 0.011 })
        ],
        edges: [
          [10, 0, 12, 'a'],
          [10, 1, 12, 'b'],
          [10, 2, 12, 'along'],
          [10, 3, 12, 'tear'],
          [12, 0, 11, 'out_0']
        ]
      } as unknown as NonNullable<Graph['groups']>[number]
    ];
    const migrated = migrateGraph(old);
    const [group] = migrated.groups!;
    const ids = [1, group.id, ...group.nodes.map((n) => n.id)];
    expect(new Set(ids).size).toBe(ids.length);
    const [points] = byType(group, POINTS);
    const [join] = byType(group, JOIN);
    expect(group.edges).toEqual(
      expect.arrayContaining([
        [10, 1, points.id, 'plant'],
        [10, 2, points.id, 'lowest_point'],
        [points.id, 0, 12, 'b'],
        [join.id, 0, 11, 'out_0']
      ])
    );
  });
});
