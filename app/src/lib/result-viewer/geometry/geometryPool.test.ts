import {
  Group,
  type InstancedMesh,
  type Mesh,
  MeshBasicMaterial,
  MeshMatcapMaterial,
  MeshStandardMaterial,
  ShaderMaterial,
  Texture
} from 'three';
import { describe, expect, it } from 'vitest';
import {
  createInstancedGeometryPool,
  createResultPools,
  readMesh,
  sortResultItems,
  withVertexColors
} from './geometryPool';

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

  it('groups every instance by the path it sits on', () => {
    const { pool, meshes } = setup();
    const data = new Int32Array([...instances(2), 0x100_0007, 9]);
    pool.update([data]);
    expect([...meshes()[0].geometry.getAttribute('instanceGroup').array]).toEqual([7, 9]);
    pool.update([instances(3)]);
    expect([...meshes()[0].geometry.getAttribute('instanceGroup').array]).toEqual([0, 0, 0]);
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

// [14, vertices, faces, flags, ...faces, ...positions, ...normals, ...colors?, ...groups] with a
// single triangle, without colors when `color` is null
function myzelMesh(color: number | null = 0xff00_80ff, x = 0, groups = [0, 0, 0]) {
  const colors = color === null ? [] : [color, color, color];
  const data = new Int32Array(4 + 3 + 3 * 6 + colors.length + 3);
  data.set([14, 3, 1, color === null ? 0 : 1, 0, 1, 2]);
  const floats = new Float32Array(data.buffer);
  floats.set([x, 0, 0, x + 1, 0, 0, x, 1, 0], 7);
  floats.set([0, 0, 1, 0, 0, 1, 0, 0, 1], 16);
  data.set([...colors, ...groups], 25);
  return data;
}

// [1, vertices, faces, path id, ...faces, ...positions, ...normals] with a single triangle
function plantGeometry(pathId: number) {
  const data = new Int32Array(4 + 3 + 3 * 6);
  data.set([1, 3, 1, pathId, 0, 1, 2]);
  new Float32Array(data.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 7);
  return data;
}

function shownMeshes(group: Group) {
  return group.children.flatMap((pool) => pool.children as Mesh[]).filter((m) => m.visible);
}

describe('sortResultItems', () => {
  it('sorts items by tag and skips unknown ones', () => {
    const geometry = Int32Array.of(1, 0, 0, 0);
    const instance = Int32Array.of(2, 0, 0, 0, 0);
    const mesh = myzelMesh();
    const field = Int32Array.of(10, 0, 0, 0, 0, 0, 0, 0);
    const path = Int32Array.of(0, 1, 2);
    const sorted = sortResultItems([path, mesh, geometry, field, instance]);
    expect(sorted.geometries).toEqual([geometry]);
    expect(sorted.instances).toEqual([instance]);
    expect(sorted.meshes).toEqual([mesh]);
  });
});

describe('readMesh', () => {
  it('reads faces, positions, normals, colors and groups', () => {
    const mesh = readMesh(myzelMesh(0x1122_3344, 0, [0, 5, 5]));
    expect(mesh.vertexCount).toBe(3);
    expect(mesh.faceCount).toBe(1);
    expect([...mesh.faces]).toEqual([0, 1, 2]);
    expect([...mesh.positions]).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    expect([...mesh.normals]).toEqual([0, 0, 1, 0, 0, 1, 0, 0, 1]);
    // red is the lowest byte
    expect([...mesh.colors!.subarray(0, 4)]).toEqual([0x44, 0x33, 0x22, 0x11]);
    expect([...mesh.groups!]).toEqual([0, 5, 5]);
  });

  it('reads meshes without colors', () => {
    const mesh = readMesh(myzelMesh(null, 0, [1, 2, 3]));
    expect(mesh.colors).toBeUndefined();
    expect([...mesh.groups!]).toEqual([1, 2, 3]);
  });

  it('rejects data of the wrong length', () => {
    expect(() => readMesh(myzelMesh().subarray(0, 20))).toThrow(/needs 31 words/);
    expect(() => readMesh(myzelMesh(null).subarray(0, 20))).toThrow(/needs 28 words/);
  });
});

describe('createResultPools', () => {
  it('shows meshes with their colors next to plain geometry', () => {
    const group = new Group();
    const material = new MeshBasicMaterial();
    const pools = createResultPools(group, material);
    const stats = pools.update([myzelMesh(0xffff_ffff, 2)]);
    expect(stats.totalVertices).toBe(3);
    expect(stats.bounds.max.x).toBe(3);

    const meshes = shownMeshes(group);
    expect(meshes).toHaveLength(1);
    const colors = meshes[0].geometry.getAttribute('color');
    expect(colors.getX(0)).toBeCloseTo(1);
    expect((meshes[0].material as MeshBasicMaterial).vertexColors).toBe(true);
    expect(material.vertexColors).toBe(false);
  });

  it('leaves tints out that would hide the colors', () => {
    const tinted = new MeshStandardMaterial();
    tinted.onBeforeCompile = () => {};
    expect(withVertexColors(tinted).onBeforeCompile).not.toBe(tinted.onBeforeCompile);
    const matcap = new MeshMatcapMaterial({ matcap: new Texture() });
    expect((withVertexColors(matcap) as MeshMatcapMaterial).matcap).toBeNull();
    expect(matcap.matcap).not.toBeNull();
    const shader = new ShaderMaterial();
    expect(withVertexColors(shader)).toBe(shader);
  });

  it('updates the colors when only they changed', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([myzelMesh(0xffff_ffff)]);
    pools.update([myzelMesh(0xff00_0000)]);
    const mesh = shownMeshes(group)[0];
    expect(mesh.geometry.getAttribute('color').getX(0)).toBe(0);
  });

  it('shows meshes without colors with the material as it is', () => {
    const group = new Group();
    const material = new MeshMatcapMaterial({ matcap: new Texture() });
    const pools = createResultPools(group, material);
    pools.update([myzelMesh(0xffff_ffff)]);
    pools.update([myzelMesh(null)]);
    const mesh = shownMeshes(group)[0];
    expect(mesh.geometry.getAttribute('color')).toBeUndefined();
    expect(mesh.material).toBe(material);

    const other = new MeshBasicMaterial();
    pools.setMaterial(other);
    expect(mesh.material).toBe(other);
  });

  it('gives every vertex the group of its mesh', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([myzelMesh(null, 0, [0, 3, 5])]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual([0, 3, 5]);
    pools.update([myzelMesh(null, 0, [1, 1, 1])]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual([1, 1, 1]);
  });

  it('groups plant geometry by its path, keeping 24 bits of the id', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    pools.update([plantGeometry(0x7f12_3456)]);
    expect([...shownMeshes(group)[0].geometry.getAttribute('group').array]).toEqual(
      [0x12_3456, 0x12_3456, 0x12_3456]
    );
  });
});
