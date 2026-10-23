import { Group, type InstancedMesh, MeshBasicMaterial } from 'three';
import { describe, expect, it } from 'vitest';
import { createInstancedGeometryPool } from './geometryPool';

// [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices]
// with a single triangle, every instance moved along x by its index times `offset`
function instances(count: number, vertexCount = 3, offset = 1) {
  const faces = [0, 1, 2];
  const positions = Array.from({ length: vertexCount * 3 }, (_, i) => i % 3 === 0 ? i / 3 : 0);
  const normals = Array.from({ length: vertexCount * 3 }, (_, i) => i % 3 === 1 ? 1 : 0);
  const matrices = Array.from({ length: count }, (_, i) => [
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
    i * offset,
    0,
    0,
    1
  ]).flat();

  const data = new Int32Array(5 + 3 + vertexCount * 6 + count * 16);
  data.set([2, vertexCount, 1, count, 0, ...faces]);
  new Float32Array(data.buffer).set([...positions, ...normals, ...matrices], 8);
  return data;
}

function setup() {
  const group = new Group();
  const pool = createInstancedGeometryPool(group, new MeshBasicMaterial());
  const meshes = () => group.children[0].children as InstancedMesh[];
  return { pool, meshes };
}

describe('createInstancedGeometryPool', () => {
  it('keeps every input on its own mesh when a mesh has to grow', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2), instances(2, 4)]);
    pool.update([instances(8), instances(2, 4)]);

    const shown = meshes().map((m) => [m.count, m.geometry.userData.vertexCount]);
    expect(shown).toContainEqual([8, 3]);
    expect(shown).toContainEqual([2, 4]);
  });

  it('shows a mesh again that was hidden for a graph with fewer inputs', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2), instances(2)]);
    pool.update([instances(2)]);
    pool.update([instances(2), instances(2)]);

    expect(meshes().every((m) => m.visible)).toBe(true);
  });

  it('bounds the instances moved by their matrices', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2, 3, 10)]);

    // the triangle spans x from 0 to 2, the second instance sits at x = 10
    expect(meshes()[0].boundingBox?.max.x).toBe(12);
    expect(meshes()[0].boundingSphere?.center.x).toBe(6);
  });

  it('reports the bounds of what is shown and nothing hidden', () => {
    const { pool } = setup();
    pool.update([instances(2, 3, 10), instances(2, 4)]);
    expect(pool.update([instances(2, 4)]).bounds.max.x).toBe(4);
  });

  it('keeps the other inputs on their meshes when one is removed', () => {
    const { pool, meshes } = setup();
    pool.update([instances(2, 3), instances(2, 4), instances(2, 5)]);
    const [a, , c] = meshes();
    pool.update([instances(2, 3), instances(2, 5)]);

    expect(meshes()[0]).toBe(a);
    expect(meshes()[2]).toBe(c);
    expect(meshes().map((m) => m.visible)).toEqual([true, false, true]);
  });
});
