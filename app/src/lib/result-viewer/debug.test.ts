import { concatEncodedArrays } from '@nodarium/utils';
import { Group, InstancedMesh, Line, Points } from 'three';
import { describe, expect, it } from 'vitest';
import { updateDebugScene } from './debug';

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

// [0, depth, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(2 + points.length * 4);
  data.set([0, depth]);
  new Float32Array(data.buffer).set(points.flat(), 2);
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
  const meshes: InstancedMesh[] = [];
  group.traverse((child) => {
    if (child instanceof InstancedMesh) meshes.push(child);
    else if (child instanceof Points) points.push(child);
    else if (child instanceof Line && !('isLineSegments' in child)) lines.push(child);
  });
  return { lines, points, meshes };
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

    const { lines, points, meshes } = collect(group);
    expect(lines).toHaveLength(2);
    expect(points[0].geometry.getAttribute('position').count).toBe(5);
    expect(meshes).toHaveLength(1);
    expect(meshes[0].count).toBe(4);
  });

  it('draws a single geometry that is not wrapped in a list', () => {
    const group = new Group();
    updateDebugScene(group, { 1: { type: 'path', data: wrap(instances(2)) } });
    expect(collect(group).meshes[0].count).toBe(2);
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
