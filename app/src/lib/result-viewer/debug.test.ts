import { concatEncodedArrays } from '@nodarium/utils';
import { ArrowHelper, Group, Line, LineSegments, Points, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { updateDebugScene } from './debug';

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

// [0, depth, id, parent id, parent alpha, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(5 + points.length * 4);
  data.set([0, depth, 0, -1, 0]);
  new Float32Array(data.buffer).set(points.flat(), 5);
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

  it('skips single values and clears the previous result', () => {
    const group = new Group();
    updateDebugScene(group, {
      1: { type: 'path', data: wrap(path(1, [[0, 0, 0, 1], [0, 1, 0, 1]])) }
    });
    expect(group.children).not.toHaveLength(0);

    updateDebugScene(group, { 1: { type: 'float', data: new Int32Array([5]) } });
    expect(group.children).toHaveLength(0);
  });
});
