import { describe, expect, it } from 'vitest';
import {
  COLORED_PATH,
  isPath,
  NO_PARENT,
  NO_PATH,
  PATH_HEADER_SIZE,
  pathLineage,
  pathPointCount,
  POINT_VALUES,
  readGeometryPathId,
  readInstanceCreator,
  readInstancePathIds,
  readPathColors,
  readPathLink
} from './pathData';

function path(
  link: {
    id: number;
    parentId: number;
    alpha: number;
    creator: number;
    index: number;
    grown?: number;
  }
) {
  const data = new Int32Array(PATH_HEADER_SIZE + 8);
  const f32 = new Float32Array(data.buffer);
  data.set([0, 2, link.id, link.parentId]);
  f32[4] = link.alpha;
  data.set([link.creator, link.index], 5);
  f32[7] = link.grown ?? 1;
  return data;
}

describe('readPathLink', () => {
  it('reads the id, the parent and who made the path', () => {
    const link = readPathLink(
      path({ id: 31, parentId: 7, alpha: 0.25, creator: 12, index: 3, grown: 0.5 })
    );

    expect(link).toEqual({
      id: 31,
      parentId: 7,
      parentAlpha: 0.25,
      creator: 12,
      index: 3,
      grown: 0.5
    });
  });

  it('reads a path that is a view into a larger buffer', () => {
    const inner = path({ id: 5, parentId: NO_PARENT, alpha: 0, creator: 1, index: 0 });
    const buffer = new Int32Array(inner.length + 3);
    buffer.set(inner, 3);

    const link = readPathLink(buffer.subarray(3));

    expect(link.id).toBe(5);
    expect(link.parentId).toBe(NO_PARENT);
  });
});

describe('readGeometryPathId', () => {
  it('reads the path the geometry was made from', () => {
    expect(readGeometryPathId(Int32Array.of(1, 3, 1, 42, 0))).toBe(42);
    expect(readGeometryPathId(Int32Array.of(1, 3, 1, NO_PATH, 0))).toBe(NO_PATH);
  });
});

describe('readInstancePathIds', () => {
  it('finds the ids after the matrices and the creator in the header', () => {
    const vertices = 3;
    const faces = 1;
    const count = 2;
    const data = new Int32Array(5 + faces * 3 + vertices * 6 + count * 16 + count);
    data.set([2, vertices, faces, count, 41]);
    data.set([10, 20], data.length - count);

    expect([...readInstancePathIds(data)]).toEqual([10, 20]);
    expect(readInstanceCreator(data)).toBe(41);
  });
});

describe('pathLineage', () => {
  const link = (id: number, parentId: number) => ({
    id,
    parentId,
    parentAlpha: 0,
    creator: 0,
    index: 0
  });

  it('walks from a path up to the one that grows on its own', () => {
    const links = [link(1, NO_PARENT), link(2, 1), link(3, 2), link(4, 1)];

    expect(pathLineage(links, 3)).toEqual([3, 2, 1]);
    expect(pathLineage(links, 4)).toEqual([4, 1]);
    expect(pathLineage(links, 1)).toEqual([1]);
  });

  it('stops at a parent that is gone and does not loop', () => {
    expect(pathLineage([link(2, 9)], 2)).toEqual([2]);
    expect(pathLineage([link(1, 2), link(2, 1)], 1)).toEqual([1, 2]);
    expect(pathLineage([], 1)).toEqual([]);
  });
});

describe('paths with colors', () => {
  it('has its colors after its points', () => {
    const colored = new Int32Array(PATH_HEADER_SIZE + 2 * 5);
    colored[0] = COLORED_PATH;
    colored.set([0x11223344, 0x55667788], PATH_HEADER_SIZE + 8);
    const plain = path({ id: 1, parentId: NO_PARENT, alpha: 0, creator: 1, index: 0 });

    expect(isPath(colored)).toBe(true);
    expect(pathPointCount(colored)).toBe(2);
    expect(pathPointCount(plain)).toBe(2);
    expect(readPathColors(plain)).toBeUndefined();
    expect([...readPathColors(colored)!]).toEqual([0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55]);
  });

  it('has its colors, fibres and fill after its points', () => {
    const all = new Int32Array(PATH_HEADER_SIZE + 3 * 7);
    all[0] = POINT_VALUES.colors | POINT_VALUES.fibres | POINT_VALUES.fill;
    all.set([1, 2, 3], PATH_HEADER_SIZE + 12);
    const fibres = new Int32Array(PATH_HEADER_SIZE + 3 * 5);
    fibres[0] = POINT_VALUES.fibres;

    expect(isPath(all)).toBe(true);
    expect(pathPointCount(all)).toBe(3);
    expect([...readPathColors(all)!]).toEqual([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    expect(pathPointCount(fibres)).toBe(3);
    expect(readPathColors(fibres)).toBeUndefined();
    expect(isPath(Int32Array.from({ length: PATH_HEADER_SIZE + 4 }, (_, i) => (i ? 0 : 128)))).toBe(
      false
    );
  });
});
