import type { Edge, NodeInstance } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import {
  collectForceGizmos,
  FORCE_NODE_TYPE,
  gizmoProps,
  radiusFromScale,
  VEC3_NODE_TYPE
} from './gizmos';

function node(id: number, props: Record<string, unknown> = {}, type = FORCE_NODE_TYPE) {
  return { id, type, props, position: [0, 0], state: {} } as unknown as NodeInstance;
}

const defaults = (_node: NodeInstance, input: string) =>
  ({ position: [0, 3, 0], radius: 3 } as Record<string, unknown>)[input];

describe('collectForceGizmos', () => {
  it('reads position and radius from the props', () => {
    const gizmos = collectForceGizmos(
      [node(1, { position: [1, 2, 3], radius: 4 })],
      [],
      1,
      defaults
    );
    expect(gizmos).toEqual([{ nodeId: 1, position: [1, 2, 3], radius: 4 }]);
  });

  it('falls back to the defaults of the node for props that are not set', () => {
    const [gizmo] = collectForceGizmos([node(1)], [], 1, defaults);
    expect(gizmo.position).toEqual([0, 3, 0]);
    expect(gizmo.radius).toBe(3);
  });

  it('gives only the active node a gizmo', () => {
    const nodes = [node(1), node(2)];
    expect(collectForceGizmos(nodes, [], 2, defaults).map((g) => g.nodeId)).toEqual([2]);
    expect(collectForceGizmos(nodes, [], -1, defaults)).toEqual([]);
  });

  it('ignores other nodes', () => {
    expect(collectForceGizmos([node(1, {}, 'max/plantarium/stem')], [], 1, defaults)).toEqual([]);
  });

  it('leaves out nodes whose position or radius is connected', () => {
    const target = node(1);
    const edges = [[node(9), 0, target, 'position']] as unknown as Edge[];
    expect(collectForceGizmos([target], edges, 1, defaults)).toEqual([]);
    expect(collectForceGizmos([node(2)], edges, 2, defaults)).toHaveLength(1);
  });

  it('keeps nodes whose strength is connected', () => {
    const target = node(1);
    const edges = [[node(9), 0, target, 'strength']] as unknown as Edge[];
    expect(collectForceGizmos([target], edges, 1, defaults)).toHaveLength(1);
  });

  it('ignores a position that is not three numbers', () => {
    const [gizmo] = collectForceGizmos([node(1, { position: [1, 2] })], [], 1, defaults);
    expect(gizmo.position).toEqual([0, 3, 0]);
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

describe('vec3 nodes', () => {
  const vec3Defaults = () => 0;

  it('only show a gizmo while they are the active node', () => {
    const nodes = [node(1, {}, VEC3_NODE_TYPE), node(2, {}, VEC3_NODE_TYPE)];
    expect(collectForceGizmos(nodes, [], -1, vec3Defaults)).toEqual([]);
    const [gizmo] = collectForceGizmos(nodes, [], 2, vec3Defaults);
    expect(gizmo).toEqual({ nodeId: 2, position: [0, 0, 0] });
  });

  it('read the position from the three inputs', () => {
    const [gizmo] = collectForceGizmos(
      [node(1, { '0': 1, '1': 2, '2': 3 }, VEC3_NODE_TYPE)],
      [],
      1,
      vec3Defaults
    );
    expect(gizmo.position).toEqual([1, 2, 3]);
    expect(gizmo.radius).toBeUndefined();
  });

  it('have no gizmo when one of the inputs is connected', () => {
    const target = node(1, {}, VEC3_NODE_TYPE);
    const edges = [[node(9), 0, target, '1']] as unknown as Edge[];
    expect(collectForceGizmos([target], edges, 1, vec3Defaults)).toEqual([]);
  });
});

describe('gizmoProps', () => {
  it('writes a position into the three inputs of a vec3 node', () => {
    expect(gizmoProps(node(1, {}, VEC3_NODE_TYPE), { position: [1, 2, 3] })).toEqual({
      '0': 1,
      '1': 2,
      '2': 3
    });
  });

  it('writes position and radius of a force node', () => {
    expect(gizmoProps(node(1), { position: [1, 2, 3] })).toEqual({ position: [1, 2, 3] });
    expect(gizmoProps(node(1), { radius: 2 })).toEqual({ radius: 2 });
  });
});
