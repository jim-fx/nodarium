import { encodeNestedArray } from '@nodarium/utils';
import { describe, expect, it } from 'vitest';
import {
  decodeResult,
  GEOMETRY_FLAGS,
  NO_CREATOR,
  readGeometry,
  readInstances,
  sortResultItems
} from './decode-result';

// [1, vertices, faces, path id, flags, ...faces, ...positions, ...normals, ...colors?,
// ...groups?] with a single triangle, without colors when `color` is null and without groups
// when `groups` is null
function geometry(
  {
    pathId = -1,
    color = 0xff00_80ff as number | null,
    x = 0,
    groups = [0, 0, 0] as number[] | null
  } = {}
) {
  const colors = color === null ? [] : [color, color, color];
  const flags = (color === null ? 0 : GEOMETRY_FLAGS.colors)
    | (groups === null ? 0 : GEOMETRY_FLAGS.groups);
  const data = new Int32Array(5 + 3 + 3 * 6 + colors.length + (groups?.length ?? 0));
  data.set([1, 3, 1, pathId, flags, 0, 1, 2]);
  const floats = new Float32Array(data.buffer);
  floats.set([x, 0, 0, x + 1, 0, 0, x, 1, 0], 8);
  floats.set([0, 0, 1, 0, 0, 1, 0, 0, 1], 17);
  data.set([...colors, ...(groups ?? [])], 26);
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
    const plain = geometry({ groups: null });
    const instance = Int32Array.of(2, 0, 0, 0, 0);
    const grouped = geometry();
    const unknown = Int32Array.of(10, 0, 0, 0, 0, 0, 0, 0);
    const path = Int32Array.of(0, 1, 2);
    const sorted = sortResultItems([path, grouped, plain, unknown, instance]);
    expect(sorted.geometries).toEqual([grouped, plain]);
    expect(sorted.instances).toEqual([instance]);
  });
});

describe('readGeometry', () => {
  it('reads the colors and the groups when the flags say so', () => {
    const read = readGeometry(geometry({ color: 0x1122_3344, groups: [0, 5, 5] }));
    expect([...read.colors!.subarray(0, 4)]).toEqual([0x44, 0x33, 0x22, 0x11]);
    expect([...read.groups!]).toEqual([0, 5, 5]);
    expect([...read.positions]).toEqual([0, 0, 0, 1, 0, 0, 0, 1, 0]);
    expect(read.creators).toBeUndefined();
  });

  it('reads a plant geometry without groups as one, with its path', () => {
    const read = readGeometry(geometry({ pathId: 5, color: null, groups: null }));
    expect(read.pathId).toBe(5);
    expect(read.colors).toBeUndefined();
    expect(read.groups).toBeUndefined();
  });

  it('throws when the words are not as many as the flags need', () => {
    expect(() => readGeometry(geometry().subarray(0, 20))).toThrow(/needs 32 words/);
    expect(() => readGeometry(geometry({ color: null }).subarray(0, 20))).toThrow(/needs 29 words/);
  });

  it('reads the creators of the groups after them', () => {
    const plain = geometry({ color: null, groups: [0, 1, 1] });
    const made = Int32Array.of(...plain, 2, 40, NO_CREATOR);
    made[4] |= GEOMETRY_FLAGS.creators;
    const read = readGeometry(made);
    expect([...read.groups!]).toEqual([0, 1, 1]);
    expect([...read.creators!]).toEqual([40, NO_CREATOR]);
    expect(() => readGeometry(made.subarray(0, made.length - 1))).toThrow(/needs 32 words, got 31/);
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

describe('decodeResult', () => {
  it('decodes every kind of item of a result', () => {
    const result = Int32Array.from(
      encodeNestedArray([
        [...geometry({ pathId: 5, color: null, groups: null })],
        [...instances(3, [5, 5, 6])],
        [...geometry({ color: null, groups: [1, 2, 3] })],
        [12, 0]
      ])
    );
    const decoded = decodeResult(result);
    expect(decoded.geometries.map(g => g.pathId)).toEqual([5, -1]);
    expect(decoded.instances.map(i => i.instanceCount)).toEqual([3]);
    expect([...decoded.geometries[1].groups!]).toEqual([1, 2, 3]);
    expect(decoded.vertexCount).toBe(3 + 3 * 3 + 3);
    expect(decoded.faceCount).toBe(1 + 3 + 1);
  });

  it('decodes an empty result', () => {
    expect(decodeResult(new Int32Array())).toMatchObject({
      geometries: [],
      instances: [],
      vertexCount: 0
    });
  });
});
