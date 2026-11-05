import { concatEncodedArrays } from '@nodarium/utils';
import { ArrowHelper, Group, Line, LineSegments, type Mesh, Points, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { updateDebugScene } from './debug';

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

// [0, depth, id, parent id, parent alpha, creator, index, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(7 + points.length * 4);
  data.set([0, depth, 0, -1, 0, 0, 0]);
  new Float32Array(data.buffer).set(points.flat(), 7);
  return data;
}

// [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices]
function instances(count: number) {
  const data = new Int32Array(5 + 3 + 18 + count * 16);
  data.set([2, 3, 1, count, 0, 0, 1, 2]);
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  new Float32Array(data.buffer).set(
    [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, ...Array(count).fill(identity).flat()],
    8
  );
  return data;
}

// [4, parent id, depth, index, x y z, dx dy dz, tx ty tz, scale, alpha]
function point(position: number[], direction: number[], scale = 0.1) {
  const data = new Int32Array(15);
  data.set([4, 7, 1, 0]);
  new Float32Array(data.buffer).set([...position, ...direction, 0, 1, 0, scale, 0.5], 4);
  return data;
}

function collect(group: Group) {
  const lines: Line[] = [];
  const points: Points[] = [];
  const segments: LineSegments[] = [];
  const arrows: ArrowHelper[] = [];
  group.traverse((child) => {
    if (child instanceof ArrowHelper) arrows.push(child);
    else if (child instanceof Points) points.push(child);
    else if (child instanceof LineSegments) segments.push(child);
    else if (child instanceof Line && !(child.parent instanceof ArrowHelper)) lines.push(child);
  });
  return { lines, points, segments, arrows };
}

describe('updateDebugScene', () => {
  it('draws only the paths of a path output as lines, instances separately', () => {
    const group = new Group();
    const data = concatEncodedArrays([
      wrap(path(1, [[0, 0, 0, 0.1], [0, 1, 0, 0.1]])),
      wrap(path(2, [[0, 1, 0, 0.05], [1, 1, 0, 0.05], [2, 1, 0, 0.05]])),
      wrap(instances(4))
    ]);

    updateDebugScene(group, { 1: { type: 'path', data } });

    const { lines, points, arrows } = collect(group);
    expect(lines).toHaveLength(2);
    expect(points[0].geometry.getAttribute('position').count).toBe(5);
    // an arrow for every instance
    expect(arrows).toHaveLength(4);
  });

  it('draws a single geometry that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { 1: { type: 'path', data: wrap(instances(2)) } });
    expect(collect(group).arrows).toHaveLength(2);
  });

  it('points an instance along its -z axis, as long as its size, scaled up', () => {
    const group = new Group();
    const data = instances(2);
    // scale 2 at (1, 2, 3) and scale 1 at (1, 2, 13)
    new Float32Array(data.buffer).set(
      [
        2,
        0,
        0,
        0,
        0,
        2,
        0,
        0,
        0,
        0,
        2,
        0,
        1,
        2,
        3,
        1,
        1,
        0,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        0,
        1,
        0,
        1,
        2,
        13,
        1
      ],
      26
    );
    updateDebugScene(group, { 1: { type: 'path', data: wrap(data) } });

    const arrows = collect(group).arrows;
    expect(arrows.map((a) => a.position.toArray())).toEqual([[1, 2, 3], [1, 2, 13]]);
    // an arrow points along +y until it is rotated
    for (const arrow of arrows) {
      const direction = new Vector3(0, 1, 0).applyQuaternion(arrow.quaternion);
      expect(direction.distanceTo(new Vector3(0, 0, -1))).toBeLessThan(1e-6);
    }
    expect(arrows.map((a) => a.cone.position.y)).toEqual([200, 100]);
  });

  it('draws a dot and an arrow along its direction for every point', () => {
    const group = new Group();
    const data = concatEncodedArrays([
      wrap(point([0, 1, 0], [1, 0, 0])),
      wrap(point([0, 2, 0], [-1, 0, 0], 0.2))
    ]);

    updateDebugScene(group, { 1: { type: 'points', data } });

    const { points, arrows, segments } = collect(group);
    expect(points[0].geometry.getAttribute('position').count).toBe(2);
    expect(arrows.map((a) => a.position.toArray())).toEqual([[0, 1, 0], [0, 2, 0]]);
    const directions = arrows.map((a) => new Vector3(0, 1, 0).applyQuaternion(a.quaternion));
    expect(directions[0].distanceTo(new Vector3(1, 0, 0))).toBeLessThan(1e-6);
    expect(directions[1].distanceTo(new Vector3(-1, 0, 0))).toBeLessThan(1e-6);
    // a thicker path gets a longer arrow
    expect(arrows[0].cone.position.y).toBeCloseTo(0.5);
    expect(arrows[1].cone.position.y).toBeCloseTo(1);
    // a line along the tangent of every point
    expect(segments[0].geometry.getAttribute('position').count).toBe(4);
  });

  it('draws a single point that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { 1: { type: 'points', data: wrap(point([0, 0, 0], [0, 0, 1])) } });
    expect(collect(group).arrows).toHaveLength(1);
  });

  it('skips single values and clears the previous result', () => {
    const group = new Group();
    updateDebugScene(group, {
      1: { type: 'path', data: wrap(path(1, [[0, 0, 0, 1], [0, 1, 0, 1]])) }
    });
    expect(group.children).not.toHaveLength(0);

    updateDebugScene(group, { 1: { type: 'float', data: new Int32Array([5]) } });
    expect(group.children).toHaveLength(0);
  });

  it('draws a Myzel mesh and the preview of a field, but not a raw field', () => {
    // [14, vertices, faces, ...faces, ...positions, ...normals, ...colors] with one triangle
    const mesh = new Int32Array(3 + 3 + 21);
    mesh.set([14, 3, 1, 0, 1, 2]);
    new Float32Array(mesh.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 6);
    const meshes = (group: Group) =>
      group.children.flatMap((g) => g.children.flatMap((pool) => pool.children as Mesh[]));

    const group = new Group();
    updateDebugScene(group, { 1: { type: 'mesh', data: mesh } });
    expect(meshes(group)).toHaveLength(1);
    expect(meshes(group)[0].geometry.getAttribute('position').count).toBe(3);

    // the runtime turns a field into the list of the output node
    updateDebugScene(group, { 1: { type: 'field', data: wrap(mesh) } });
    expect(meshes(group)).toHaveLength(1);

    updateDebugScene(group, { 1: { type: 'field', data: Int32Array.of(10, 0, 0, 0, 0, 0, 0, 0) } });
    expect(group.children).toHaveLength(0);
  });
});
