import { encodeNestedArray } from '@nodarium/utils';
import { describe, expect, it } from 'vitest';
import {
  decodeResult,
  readGeometry,
  readInstances,
  readMesh,
  sortResultItems
} from './decode-result';

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

// [1, vertices, faces, path id, ...faces, ...positions, ...normals, ...colors?] with a single
// triangle
function plantGeometry(pathId: number, colors: number[] = []) {
  const data = new Int32Array(4 + 3 + 3 * 6 + colors.length);
  data.set([1, 3, 1, pathId, 0, 1, 2]);
  new Float32Array(data.buffer).set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 7);
  data.set(colors, 25);
  return data;
}

// [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices,
// ...path ids, ...colors?] with a single triangle, every instance moved along x by its index
function instances(count: number, pathIds: number[] = [], colors: number[] = []) {
  const data = new Int32Array(5 + 3 + 3 * 6 + count * 16 + pathIds.length + colors.length);
  data.set([2, 3, 1, count, 1, 0, 1, 2]);
  const floats = new Float32Array(data.buffer);
  floats.set([0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1], 8);
  for (let i = 0; i < count; i++) {
    floats.set([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, i, 0, 0, 1], 26 + i * 16);
  }
  data.set(pathIds, 26 + count * 16);
  data.set(colors, 26 + count * 16 + pathIds.length);
  return data;
}

describe('sortResultItems', () => {
  it('sorts items by tag and skips unknown ones', () => {
    const geometry = Int32Array.of(1, 0, 0, 0);
    const instance = Int32Array.of(2, 0, 0, 0, 0);
    const mesh = myzelMesh();
    const unknown = Int32Array.of(10, 0, 0, 0, 0, 0, 0, 0);
    const path = Int32Array.of(0, 1, 2);
    const sorted = sortResultItems([path, mesh, geometry, unknown, instance]);
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

describe('readInstances', () => {
  it('reads the shape, the matrices and the path ids', () => {
    const read = readInstances(instances(2, [7, 9]));
    expect(read.instanceCount).toBe(2);
    expect(read.depth).toBe(1);
    expect([...read.faces]).toEqual([0, 1, 2]);
    expect([...read.positions]).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    expect(read.matrices).toHaveLength(32);
    expect(read.matrices[16 + 12]).toBe(1);
    expect([...read.pathIds]).toEqual([7, 9]);
  });

  it('gives instances without a path id NO_PATH', () => {
    expect([...readInstances(instances(2)).pathIds]).toEqual([-1, -1]);
  });

  it('reads the colors of the shape after the path ids', () => {
    expect(readInstances(instances(2, [7, 9])).colors).toBeUndefined();
    const read = readInstances(instances(2, [7, 9], [0x11223344, -1, 0x00ffffff]));
    expect([...read.pathIds]).toEqual([7, 9]);
    expect([...read.colors!]).toEqual([
      0x44,
      0x33,
      0x22,
      0x11,
      255,
      255,
      255,
      255,
      255,
      255,
      255,
      0
    ]);
  });
});

describe('readGeometry', () => {
  it('reads a color per vertex when there is one', () => {
    expect(readGeometry(plantGeometry(5)).colors).toBeUndefined();
    const read = readGeometry(plantGeometry(5, [0x11223344, 0x11223344, -1]));
    expect([...read.colors!.subarray(0, 4)]).toEqual([0x44, 0x33, 0x22, 0x11]);
    expect([...read.positions]).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
  });
});

describe('decodeResult', () => {
  it('decodes every kind of item of a result', () => {
    const result = Int32Array.from(
      encodeNestedArray([
        [...plantGeometry(5)],
        [...instances(3, [5, 5, 6])],
        [...myzelMesh(null, 0, [1, 2, 3])],
        [12, 0]
      ])
    );
    const decoded = decodeResult(result);
    expect(decoded.geometries.map(g => g.pathId)).toEqual([5]);
    expect(decoded.instances.map(i => i.instanceCount)).toEqual([3]);
    expect([...decoded.meshes[0].groups!]).toEqual([1, 2, 3]);
    expect(decoded.vertexCount).toBe(3 + 3 * 3 + 3);
    expect(decoded.faceCount).toBe(1 + 3 + 1);
  });

  it('decodes an empty result', () => {
    expect(decodeResult(new Int32Array())).toMatchObject({
      geometries: [],
      instances: [],
      meshes: [],
      vertexCount: 0
    });
  });
});
