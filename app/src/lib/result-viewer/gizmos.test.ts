import type { Edge, Graph, NodeInstance } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import type { ProbePath } from './gizmo-paths';
import {
  addProbe,
  BRANCH_NODE_TYPE,
  collectGizmos,
  FORCE_NODE_TYPE,
  probeSource,
  radiusFromScale,
  ROTATE_NODE_TYPE,
  STEM_NODE_TYPE,
  VEC3_NODE_TYPE
} from './gizmos';

function node(id: number, props: Record<string, unknown> = {}, type = FORCE_NODE_TYPE) {
  return { id, type, props, position: [0, 0], state: {} } as unknown as NodeInstance;
}

const defaults = (_node: NodeInstance, input: string) =>
  ({ position: [0, 3, 0], radius: 3, origin: [0, 0, 0], length: 5 } as Record<string, unknown>)[
    input
  ];

/** an edge from some other node into an input of `target` */
function edgeTo(target: NodeInstance, input: string) {
  return [node(99), 0, target, input] as unknown as Edge;
}

describe('force nodes', () => {
  it('read position and radius from the props', () => {
    const gizmos = collectGizmos(
      [node(1, { position: [1, 2, 3], radius: 4 })],
      [],
      1,
      defaults
    );
    expect(gizmos).toEqual([{ kind: 'force', nodeId: 1, position: [1, 2, 3], radius: 4 }]);
  });

  it('fall back to the defaults of the node for props that are not set', () => {
    const [gizmo] = collectGizmos([node(1)], [], 1, defaults);
    expect(gizmo).toMatchObject({ position: [0, 3, 0], radius: 3 });
  });

  it('give only the active node a gizmo', () => {
    const nodes = [node(1), node(2)];
    expect(collectGizmos(nodes, [], 2, defaults).map((g) => g.nodeId)).toEqual([2]);
    expect(collectGizmos(nodes, [], -1, defaults)).toEqual([]);
  });

  it('have no gizmo when the position or radius is connected', () => {
    const target = node(1);
    expect(collectGizmos([target], [edgeTo(target, 'position')], 1, defaults)).toEqual([]);
    expect(collectGizmos([target], [edgeTo(target, 'radius')], 1, defaults)).toEqual([]);
  });

  it('keep their gizmo when the strength is connected', () => {
    const target = node(1);
    expect(collectGizmos([target], [edgeTo(target, 'strength')], 1, defaults)).toHaveLength(1);
  });

  it('ignore a position that is not three numbers', () => {
    const [gizmo] = collectGizmos([node(1, { position: [1, 2] })], [], 1, defaults);
    expect(gizmo).toMatchObject({ position: [0, 3, 0] });
  });

  it('are not found on other nodes', () => {
    expect(collectGizmos([node(1, {}, 'max/plantarium/noise')], [], 1, defaults)).toEqual([]);
  });
});

describe('vec3 nodes', () => {
  const vec3Defaults = () => 0;

  it('read the position from the three inputs', () => {
    const [gizmo] = collectGizmos(
      [node(1, { '0': 1, '1': 2, '2': 3 }, VEC3_NODE_TYPE)],
      [],
      1,
      vec3Defaults
    );
    expect(gizmo).toEqual({ kind: 'position', nodeId: 1, position: [1, 2, 3] });
  });

  it('have no gizmo when one of the inputs is connected', () => {
    const target = node(1, {}, VEC3_NODE_TYPE);
    expect(collectGizmos([target], [edgeTo(target, '1')], 1, vec3Defaults)).toEqual([]);
  });
});

describe('stem nodes', () => {
  it('show the origin and the length', () => {
    const [gizmo] = collectGizmos(
      [node(1, { origin: [1, 0, 2], length: 7 }, STEM_NODE_TYPE)],
      [],
      1,
      defaults
    );
    expect(gizmo).toEqual({ kind: 'stem', nodeId: 1, origin: [1, 0, 2], length: 7 });
  });

  it('use the defaults for what is not set', () => {
    const [gizmo] = collectGizmos([node(1, {}, STEM_NODE_TYPE)], [], 1, defaults);
    expect(gizmo).toMatchObject({ origin: [0, 0, 0], length: 5 });
  });

  it('leave out the length handle when the length is connected', () => {
    const target = node(1, {}, STEM_NODE_TYPE);
    const [gizmo] = collectGizmos([target], [edgeTo(target, 'length')], 1, defaults);
    expect(gizmo).toMatchObject({ kind: 'stem' });
    expect(gizmo).toHaveProperty('length', undefined);
  });

  it('have no gizmo when the origin is connected', () => {
    const target = node(1, {}, STEM_NODE_TYPE);
    expect(collectGizmos([target], [edgeTo(target, 'origin')], 1, defaults)).toEqual([]);
  });
});

const stemPath: ProbePath = {
  depth: 1,
  points: [[1, 0, 2], [1, 1, 2], [1, 2, 2], [1, 3, 2], [1, 4, 2]]
};

describe('rotate nodes', () => {
  const rotate = (props: Record<string, unknown> = {}) => node(1, props, ROTATE_NODE_TYPE);

  it('sit at the first point of the incoming paths', () => {
    const [gizmo] = collectGizmos([rotate({ axis: 2, angle: 1.5 })], [], 1, defaults, [stemPath]);
    expect(gizmo).toEqual({ kind: 'rotate', nodeId: 1, origin: [1, 0, 2], axis: 2, angle: 1.5 });
  });

  it('wait for the paths', () => {
    expect(collectGizmos([rotate()], [], 1, defaults, [])).toEqual([]);
  });

  it('have no gizmo when the angle is connected', () => {
    const target = rotate();
    expect(collectGizmos([target], [edgeTo(target, 'angle')], 1, defaults, [stemPath])).toEqual([]);
  });
});

describe('branch nodes', () => {
  const branch = (props: Record<string, unknown> = {}) => node(1, props, BRANCH_NODE_TYPE);

  it('start where the first branch grows on the path', () => {
    // a vertical stem, lowest branch at 30% of its length
    const [gizmo] = collectGizmos(
      [branch({ lowestBranch: 0.3, angle: 20, length: 2 })],
      [],
      1,
      defaults,
      [stemPath]
    );
    if (gizmo.kind !== 'branch') throw new Error('not a branch gizmo');

    expect(gizmo.origin[1]).toBeCloseTo(1.2);
    expect(gizmo.direction).toEqual([0, 1, 0]);
    // a vertical stem gets an orthogonal along z
    expect(gizmo.orthogonal).toEqual([0, 0, -1]);
    expect(gizmo.angle).toBe(20);
    expect(gizmo.length).toBe(2);
  });

  it('use the offset of the even branches', () => {
    const [gizmo] = collectGizmos(
      [branch({ lowestBranch: 0, highestBranch: 1, amount: 5, offsetSingle: 1 })],
      [],
      1,
      defaults,
      [stemPath]
    );
    // the step is a quarter of the stem, the first branch moves up by all of it
    expect(gizmo).toMatchObject({ origin: [1, 1, 2] });
  });

  it('work on the layer the node works on, not the first path', () => {
    const twig: ProbePath = { depth: 2, points: [[5, 0, 0], [5, 1, 0]] };
    const [gizmo] = collectGizmos([branch({ lowestBranch: 0 })], [], 1, defaults, [stemPath, twig]);
    if (gizmo.kind !== 'branch') throw new Error('not a branch gizmo');
    expect(gizmo.origin[0]).toBe(5);
    expect(gizmo.origin[1]).toBeCloseTo(0);
  });

  it('have no gizmo when the angle or length is connected', () => {
    const target = branch();
    expect(collectGizmos([target], [edgeTo(target, 'angle')], 1, defaults, [stemPath])).toEqual([]);
    expect(collectGizmos([target], [edgeTo(target, 'length')], 1, defaults, [stemPath])).toEqual(
      []
    );
  });
});

describe('probeSource', () => {
  it('finds the node that feeds the paths into a node', () => {
    const edges = [[7, 1, 3, 'plant'], [8, 0, 3, 'length']] as Graph['edges'];
    expect(probeSource(ROTATE_NODE_TYPE, 3, edges)).toEqual({ nodeId: 7, output: 1 });
    expect(probeSource(BRANCH_NODE_TYPE, 3, edges)).toEqual({ nodeId: 7, output: 1 });
  });

  it('is only needed by nodes that work on paths', () => {
    const edges = [[7, 0, 3, 'plant']] as Graph['edges'];
    expect(probeSource(FORCE_NODE_TYPE, 3, edges)).toBeUndefined();
    expect(probeSource(ROTATE_NODE_TYPE, 4, edges)).toBeUndefined();
  });

  it('works with the edges of the graph manager, too', () => {
    const target = node(3, {}, ROTATE_NODE_TYPE);
    const edges = [[node(7), 0, target, 'plant']] as unknown as Edge[];
    expect(probeSource(ROTATE_NODE_TYPE, 3, edges)).toEqual({ nodeId: 7, output: 0 });
  });
});

describe('addProbe', () => {
  const graph = {
    id: 1,
    settings: {},
    nodes: [
      { id: 1, type: STEM_NODE_TYPE, position: [0, 0], props: {} },
      { id: 2, type: ROTATE_NODE_TYPE, position: [0, 0], props: {} }
    ],
    edges: [[1, 0, 2, 'plant']]
  } as unknown as Graph;

  it('puts a debug node on the source of the active node', () => {
    const probed = addProbe(graph, 2);
    expect(probed.sourceId).toBe(1);
    expect(probed.hidden).toBe(true);

    const debug = probed.graph.nodes.at(-1)!;
    expect(debug.type).toBe('__internal/node/debug');
    expect(debug.id).toBe(3);
    expect(probed.graph.edges.at(-1)).toEqual([1, 0, 3, 'input']);
    // the graph that is saved stays as it was
    expect(graph.nodes).toHaveLength(2);
    expect(graph.edges).toHaveLength(1);
  });

  it('leaves the graph alone for nodes that need nothing', () => {
    expect(addProbe(graph, 1)).toEqual({ graph });
    expect(addProbe(graph, -1)).toEqual({ graph });
  });

  it('does not add a second debug node, nor hide the first', () => {
    const watched = {
      ...graph,
      nodes: [...graph.nodes, {
        id: 3,
        type: '__internal/node/debug',
        position: [0, 0],
        props: {}
      }],
      edges: [...graph.edges, [1, 0, 3, 'input']]
    } as unknown as Graph;
    const probed = addProbe(watched, 2);
    expect(probed.graph).toBe(watched);
    expect(probed.sourceId).toBe(1);
    expect(probed.hidden).toBe(false);
  });
});

describe('radiusFromScale', () => {
  it('follows the axis that moved', () => {
    expect(radiusFromScale([3, 3, 4.5], 3)).toBe(4.5);
    expect(radiusFromScale([2, 3, 3], 3)).toBe(2);
  });

  it('keeps a radius that can be grabbed again', () => {
    expect(radiusFromScale([0, 0, 0], 3)).toBe(0.05);
  });

  it('stays the same when nothing moved', () => {
    expect(radiusFromScale([3, 3, 3], 3)).toBe(3);
  });
});
