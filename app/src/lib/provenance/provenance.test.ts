import { HIGHLIGHT } from '#lib/result-viewer/provenance/highlight.ts';
import { toCreatorId } from '@nodarium/runtime';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { NO_PARENT, NO_PATH, type PathLink } from '@nodarium/utils';
import { describe, expect, it } from 'vitest';
import {
  buildIndex,
  composeHighlight,
  downstream,
  lineageOf,
  nodeOfPart,
  nodeTraits,
  type Parts,
  partsOfNodes,
  partsOfPicked,
  partsOfRunning,
  type ResultParts,
  runningNodes,
  visibleNode,
  withDescendants
} from './provenance';

type Input = { type: string; accepts?: string[] };

function definition(
  id: string,
  outputs: NodeDefinition['outputs'],
  inputs: Record<string, Input> = {}
) {
  return { id, outputs, inputs } as unknown as NodeDefinition;
}

const creator: Input = { type: 'creator' };
const DEFINITIONS = [
  definition('p/stem', ['path'], { length: { type: 'float' }, creator }),
  definition('p/branch', [{ type: 'path', label: 'plant' }], {
    plant: { type: 'path' },
    length: { type: 'float' },
    creator
  }),
  definition('p/gravity', ['path'], { strength: { type: 'float' }, plant: { type: 'path' } }),
  definition('p/separate', ['path'], { plant: { type: 'path' }, mask: { type: 'path' } }),
  definition('p/instance', ['path'], { plant: { type: 'path' }, shape: { type: 'shape' } }),
  definition('p/leaf', ['shape']),
  definition('p/spiral', ['points'], { amount: { type: 'integer' } }),
  definition('p/instance_from_points', ['path'], {
    plant: { type: 'path' },
    points: { type: 'points' },
    shape: { type: 'shape' },
    creator
  }),
  definition('p/curve', ['float']),
  definition('p/random', ['float'], { creator }),
  definition('p/output', [], { input: { type: 'path', accepts: ['geometry'] } }),
  definition('m/strands', ['path'], { stem: { type: 'path' }, creator }),
  definition('m/cap', ['path'], { stem: { type: 'path' }, creator }),
  definition('m/lamellae', ['geometry'], { cap: { type: 'path' }, creator }),
  definition('m/skirt', ['geometry'], {
    stem: { type: 'path' },
    length: { type: 'float' },
    creator
  }),
  definition('m/hyphae_to_mesh', ['geometry'], { paths: { type: 'path' } }),
  definition('p/join', ['path'], {
    a: { type: 'path', accepts: ['geometry'] },
    b: { type: 'path', accepts: ['geometry'] }
  })
];
const byType = new Map(DEFINITIONS.map((d) => [d.id as string, d]));
const traits = nodeTraits((type) => byType.get(type));

type Node = [id: number, type: string, props?: Record<string, number>];
type EdgeOf = [from: number, output: number, to: number, input: string];

function graphOf(nodes: Node[], edges: EdgeOf[], groups: Graph['groups'] = []): Graph {
  return {
    id: 1,
    nodes: nodes.map(([id, type, props]) => ({
      id,
      type,
      position: [0, 0],
      ...(props && { props })
    })) as Graph['nodes'],
    edges,
    groups
  };
}

function link(id: number, creator: number, parentId = NO_PARENT): PathLink {
  return { id, parentId, parentAlpha: 0, creator, index: 0, grown: 1 };
}

function resultOf(links: PathLink[], tissues: number[] = []): ResultParts {
  return { links: new Map(links.map((l) => [l.id, l])), tissues: new Set(tissues) };
}

const sorted = (ids: Set<number>) => [...ids].sort((a, b) => a - b);
const plain = (parts: Parts) => ({
  paths: sorted(parts.paths),
  instances: sorted(parts.instances),
  creators: sorted(parts.creators)
});

describe('the parts of a plant', () => {
  // stem 1 → branch 2 → gravity 3 → instance 4 → output 5, a curve 7 into the length of the
  // branch, a leaf 6 into the instances
  const graph = graphOf(
    [[1, 'p/stem'], [2, 'p/branch'], [3, 'p/gravity'], [4, 'p/instance'], [5, 'p/output'], [
      6,
      'p/leaf'
    ], [7, 'p/curve']],
    [[1, 0, 2, 'plant'], [2, 0, 3, 'plant'], [3, 0, 4, 'plant'], [4, 0, 5, 'input'], [
      6,
      0,
      4,
      'shape'
    ], [7, 0, 2, 'length']]
  );
  const index = buildIndex(graph, traits);
  const result = resultOf([link(101, 1), link(201, 2, 101), link(202, 2, 101)]);
  const stemAndBranches = [101, 201, 202];

  it.each([
    ['a creator: its own paths', 1, [101], [101]],
    ['a creator: only its own paths, not those it grows on', 2, [201, 202], [201, 202]],
    ['a node that passes paths on: the paths of the creators before it', 3, stemAndBranches, [
      ...stemAndBranches
    ]],
    ['an instance node: only what sits on the paths', 4, [], stemAndBranches],
    ['a value: the paths of the node that reads it', 7, [201, 202], [201, 202]],
    ['a shape: the instances of the node that reads it', 6, [], stemAndBranches],
    ['the output: nothing, it would tint everything', 5, [], []]
  ])('%s', (_, node, paths, instances) => {
    expect(plain(partsOfRunning(index, result, [node]))).toEqual({
      paths,
      instances,
      creators: []
    });
  });

  it('leads from a path to its creator, from an instance to the one instance node', () => {
    expect(nodeOfPart(index, result, { kind: 'geometry', pathId: 201 })).toBe(2);
    expect(nodeOfPart(index, result, { kind: 'instances', pathId: 201 })).toBe(4);
    expect(nodeOfPart(index, result, { kind: 'geometry', pathId: 999 })).toBeUndefined();
  });

  it('leads from an instance that names the node that placed it to that node', () => {
    // a rosette: spiral 1 → instance_from_points 4 and 8 → output 5, a leaf 6 into 4
    const rosette = graphOf(
      [[1, 'p/spiral'], [4, 'p/instance_from_points'], [8, 'p/instance_from_points'], [
        5,
        'p/output'
      ], [6, 'p/leaf']],
      [[1, 0, 4, 'points'], [1, 0, 8, 'points'], [4, 0, 5, 'input'], [6, 0, 4, 'shape']]
    );
    const rosetteIndex = buildIndex(rosette, traits);
    const placed = resultOf([], [4, 8]);
    const hit = { kind: 'instances', pathId: NO_PATH, creator: 8 } as const;
    expect(nodeOfPart(rosetteIndex, placed, hit)).toBe(8);
    expect(nodeOfPart(rosetteIndex, placed, { ...hit, creator: undefined })).toBeUndefined();

    const only = (creators: number[]) => ({ paths: [], instances: [], creators });
    expect(plain(partsOfRunning(rosetteIndex, placed, [4]))).toEqual(only([4]));
    expect(plain(partsOfRunning(rosetteIndex, placed, [6]))).toEqual(only([4]));
    expect(plain(partsOfRunning(rosetteIndex, placed, [1]))).toEqual(only([4, 8]));
  });

  it('leads from an instance to the creator of its path when two instance nodes get it', () => {
    const two = graphOf(
      [[1, 'p/stem'], [4, 'p/instance'], [8, 'p/instance']],
      [[1, 0, 4, 'plant'], [1, 0, 8, 'plant']]
    );
    const twoIndex = buildIndex(two, traits);
    expect(nodeOfPart(twoIndex, result, { kind: 'instances', pathId: 101 })).toBe(1);
  });

  it('does not follow paths into an input that only looks at them', () => {
    // gravity 3 has no creator before it, its paths only go into the mask of separate 9
    const masked = graphOf(
      [[1, 'p/stem'], [3, 'p/gravity'], [9, 'p/separate'], [5, 'p/output']],
      [[3, 0, 9, 'mask'], [1, 0, 9, 'plant'], [9, 0, 5, 'input']]
    );
    const maskedIndex = buildIndex(masked, traits);
    expect(plain(partsOfRunning(maskedIndex, result, [3])).paths).toEqual([]);
    expect(plain(partsOfRunning(maskedIndex, result, [9])).paths).toEqual([101]);
  });

  it('gives a value that two creators read the paths of both', () => {
    const shared = graphOf(
      [[1, 'p/stem'], [2, 'p/branch'], [7, 'p/curve']],
      [[1, 0, 2, 'plant'], [7, 0, 1, 'length'], [7, 0, 2, 'length']]
    );
    const sharedIndex = buildIndex(shared, traits);
    expect(plain(partsOfRunning(sharedIndex, result, [7])).paths).toEqual(stemAndBranches);
  });

  it('finds the path, the paths it grows on and the paths that grow on it', () => {
    expect(lineageOf(result.links, 201)).toEqual([201, 101]);
    expect(sorted(withDescendants(result.links, [101]))).toEqual(stemAndBranches);
    expect(sorted(withDescendants(result.links, [201]))).toEqual([201]);
  });
});

describe('the parts of a mushroom', () => {
  // stem 10 carries the strands 1 and the skirt 3, whose tissues are in the mesh, and the cap 12
  // whose rays only carry the gills 13; a random 11 into the length of the skirt
  const graph = graphOf(
    [
      [10, 'p/stem'],
      [1, 'm/strands'],
      [2, 'm/hyphae_to_mesh'],
      [3, 'm/skirt'],
      [12, 'm/cap'],
      [13, 'm/lamellae'],
      [4, 'p/join'],
      [5, 'p/output'],
      [11, 'p/random']
    ],
    [
      [10, 0, 1, 'stem'],
      [1, 0, 2, 'paths'],
      [10, 0, 3, 'stem'],
      [10, 0, 12, 'stem'],
      [12, 0, 13, 'cap'],
      [2, 0, 4, 'a'],
      [3, 0, 4, 'b'],
      [13, 0, 4, 'b'],
      [4, 0, 5, 'input'],
      [11, 0, 3, 'length']
    ]
  );
  const index = buildIndex(graph, traits);
  const result = resultOf([], [1, 3, 13]);

  it.each([
    ['a node that made a tissue: the tissue', 3, [3]],
    ['a node whose paths only carry tissues: the nearest of them', 10, [1, 3]],
    ['a node that meshes paths: the tissues of the paths it meshes', 2, [1]],
    ['a value: the tissue of the node that reads it', 11, [3]],
    ['a maker that left nothing: the tissue of what it carries', 12, [13]],
    ['the output: nothing', 5, []]
  ])('%s', (_, node, creators) => {
    expect(plain(partsOfRunning(index, result, [node])).creators).toEqual(creators);
  });

  it('goes on through makers that left nothing when nothing is nearer', () => {
    // a stem that only carries a cap, whose rays only carry gills
    const capOnly = graphOf(
      [[10, 'p/stem'], [12, 'm/cap'], [13, 'm/lamellae']],
      [[10, 0, 12, 'stem'], [12, 0, 13, 'cap']]
    );
    const capIndex = buildIndex(capOnly, traits);
    expect(plain(partsOfRunning(capIndex, result, [10])).creators).toEqual([13]);
  });

  it('leads from a tissue to the node its creator id names', () => {
    expect(nodeOfPart(index, result, { kind: 'groups', pathId: 0, creator: 13 })).toBe(13);
    expect(nodeOfPart(index, result, { kind: 'groups', pathId: 0 })).toBeUndefined();
    expect(nodeOfPart(index, result, { kind: 'groups', pathId: 0, creator: -(2 ** 31) }))
      .toBeUndefined();
  });
});

describe('nodes inside groups', () => {
  // group node 9 holds a stem 51 of group 50, the group node 8 holds group node 60 of group 50
  const inner = (id: number): Graph['groups'][0] => ({
    id,
    nodes: [
      { id: 100, type: '__internal/group/input', position: [0, 0] },
      { id: 51, type: 'p/stem', position: [0, 0] },
      { id: 101, type: '__internal/group/output', position: [0, 0] }
    ] as Graph['nodes'],
    edges: [[51, 0, 101, 'Out']],
    inputs: {},
    outputs: [{ type: 'path', label: 'plant' }]
  });
  const outer: Graph['groups'][0] = {
    ...inner(70),
    nodes: [
      { id: 100, type: '__internal/group/input', position: [0, 0] },
      { id: 60, type: '__internal/group/instance', position: [0, 0], props: { groupId: 50 } },
      { id: 101, type: '__internal/group/output', position: [0, 0] }
    ] as Graph['nodes'],
    edges: [[60, 0, 101, 'Out']]
  };
  const graph = graphOf(
    [
      [9, '__internal/group/instance', { groupId: 50 }],
      [8, '__internal/group/instance', { groupId: 70 }],
      [5, 'p/output']
    ],
    [[9, 0, 5, 'input'], [8, 0, 5, 'input']],
    [inner(50), outer]
  );
  const index = buildIndex(graph, traits);
  const [inNine] = runningNodes(index, [], 9);
  const [inEight] = runningNodes(index, [], 8);
  const result = resultOf([link(301, toCreatorId(inNine)), link(302, toCreatorId(inEight))]);

  it('runs the nodes of a group node, and leads back to it', () => {
    expect(index.chains.get(inNine)).toEqual([9, 51]);
    expect(index.chains.get(inEight)).toEqual([8, 60, 51]);
    expect(runningNodes(index, [9], 51)).toEqual([inNine]);
    expect(runningNodes(index, [8], 60)).toEqual([inEight]);
  });

  it('shows the node on the level that is entered', () => {
    const chain = index.chains.get(inEight)!;
    expect(visibleNode(chain, [])).toBe(8);
    expect(visibleNode(chain, [8])).toBe(60);
    expect(visibleNode(chain, [8, 60])).toBe(51);
    // inside another group the node is not to be seen
    expect(visibleNode(chain, [9])).toBeUndefined();
  });

  it('tints what the nodes of a group node made, apart from the same group elsewhere', () => {
    expect(plain(partsOfNodes(index, result, [], [9])).paths).toEqual([301]);
    expect(plain(partsOfNodes(index, result, [], [8])).paths).toEqual([302]);
    expect(plain(partsOfNodes(index, result, [8], [60])).paths).toEqual([302]);
    expect(nodeOfPart(index, result, { kind: 'geometry', pathId: 302 })).toBe(inEight);
  });
});

describe('composeHighlight', () => {
  const parts = (paths: number[], creators: number[] = []): Parts => ({
    paths: new Set(paths),
    instances: new Set(paths),
    creators: new Set(creators)
  });

  it('is nothing without anything to point out', () => {
    expect(composeHighlight({})).toBeUndefined();
    expect(composeHighlight({ selected: parts([]) })).toBeUndefined();
  });

  it('tints the selection, the picked part in it stronger, the hover over both', () => {
    const highlight = composeHighlight({
      selected: parts([1, 2, 3], [40]),
      picked: parts([2]),
      hovered: parts([3]),
      selection: 'a'
    })!;
    expect([...highlight.paths]).toEqual([
      [1, HIGHLIGHT.selected],
      [2, HIGHLIGHT.focus],
      [3, HIGHLIGHT.hover]
    ]);
    expect(highlight.creators.get(40)).toBe(HIGHLIGHT.selected);
    expect(highlight.rest).toBe(HIGHLIGHT.none);
    expect(highlight.selection).toBe('a');
  });

  it('gives a selection without a picked part the strong tint', () => {
    expect(composeHighlight({ selected: parts([1]) })!.paths.get(1)).toBe(HIGHLIGHT.focus);
  });

  it('shows only the isolated parts in the local view, a hover only among them', () => {
    const highlight = composeHighlight({
      selected: parts([1, 2]),
      isolated: parts([1], [40]),
      hovered: parts([1, 2])
    })!;
    expect([...highlight.paths]).toEqual([[1, HIGHLIGHT.hover]]);
    expect(highlight.creators.get(40)).toBe(HIGHLIGHT.none);
    expect(highlight.rest).toBe(HIGHLIGHT.hidden);
  });
});

describe('partsOfPicked', () => {
  const links = new Map([link(101, 1), link(201, 2, 101)].map((l) => [l.id, l]));

  it('is the picked path, with descendants what grows on it', () => {
    const picked = { paths: [101], creators: [], instancesOnly: false };
    expect(plain(partsOfPicked(links, picked)).paths).toEqual([101]);
    expect(plain(partsOfPicked(links, picked, true)).paths).toEqual([101, 201]);
  });

  it('is only what sits on the path for a leaf that led to its instance node', () => {
    const picked = { paths: [101], creators: [7], instancesOnly: true };
    expect(plain(partsOfPicked(links, picked))).toEqual({
      paths: [],
      instances: [101],
      creators: [7]
    });
  });
});

describe('downstream', () => {
  it('is the nodes after the given ones, and those', () => {
    const edges: [number, number][] = [[1, 2], [2, 3], [4, 3], [3, 5]];
    expect(sorted(downstream(edges, [2]))).toEqual([2, 3, 5]);
    expect(sorted(downstream(edges, [5]))).toEqual([5]);
  });
});

describe('nodeTraits', () => {
  it('reads which outputs carry parts, from a type or an output with a label', () => {
    expect(traits.isPartOutput('p/stem', 0)).toBe(true);
    expect(traits.isPartOutput('p/branch', 0)).toBe(true);
    expect(traits.isPartOutput('m/lamellae', 0)).toBe(true);
    expect(traits.isPartOutput('p/curve', 0)).toBe(false);
    expect(traits.isPartOutput('p/stem', 1)).toBe(false);
    expect(traits.isPartOutput('unknown', 0)).toBe(false);
  });

  it('takes the first input that takes parts as the one a node works on', () => {
    expect(traits.isMainInput('p/gravity', 'plant')).toBe(true);
    expect(traits.isMainInput('p/separate', 'plant')).toBe(true);
    expect(traits.isMainInput('p/separate', 'mask')).toBe(false);
    expect(traits.isMainInput('p/output', 'input')).toBe(true);
    // a join works on every input alike
    expect(traits.isMainInput('p/join', 'b')).toBe(true);
  });

  it('knows the nodes the runtime gives a creator id', () => {
    expect(traits.hasCreator('p/stem')).toBe(true);
    expect(traits.hasCreator('p/random')).toBe(true);
    expect(traits.hasCreator('p/gravity')).toBe(false);
  });
});
