import { concatEncodedArrays } from '@nodarium/utils';
import { describe, expect, it } from 'vitest';
import { angleStep, pickBranchPath, readPaths, sampleAlong } from './gizmo-paths';

// [0, depth, id, parent id, parent alpha, ...x y z thickness]
function path(depth: number, points: number[][]) {
  const data = new Int32Array(5 + points.length * 4);
  data.set([0, depth, 0, -1, 0]);
  new Float32Array(data.buffer).set(points.flat(), 5);
  return data;
}

function wrap(item: Int32Array) {
  const data = new Int32Array(item.length + 4);
  data.set([0, item.length + 1]);
  data.set(item, 2);
  data.set([1, 1], item.length + 2);
  return data;
}

describe('readPaths', () => {
  it('reads depth and points of every path in a list', () => {
    const list = concatEncodedArrays([
      wrap(path(1, [[0, 0, 0, 0.1], [0, 1, 0, 0.1]])),
      wrap(path(2, [[1, 1, 1, 0.1], [2, 1, 1, 0.1], [3, 1, 1, 0.1]]))
    ]);
    const paths = readPaths(list);

    expect(paths).toHaveLength(2);
    expect(paths[0]).toEqual({ depth: 1, points: [[0, 0, 0], [0, 1, 0]] });
    expect(paths[1].points).toEqual([[1, 1, 1], [2, 1, 1], [3, 1, 1]]);
  });

  it('reads a single path that is not in a list', () => {
    const paths = readPaths(wrap(path(1, [[0, 0, 0, 0.1], [0, 1, 0, 0.1]])));
    expect(paths).toHaveLength(1);
  });

  it('has no paths in nothing', () => {
    expect(readPaths(undefined)).toEqual([]);
    expect(readPaths(new Int32Array())).toEqual([]);
  });
});

describe('pickBranchPath', () => {
  const paths = [{ depth: 1, points: [] }, { depth: 2, points: [] }, { depth: 3, points: [] }];

  it('takes the first path of the outermost layers', () => {
    expect(pickBranchPath(paths, 1)).toBe(paths[2]);
    expect(pickBranchPath(paths, 2)).toBe(paths[1]);
    expect(pickBranchPath(paths, 3)).toBe(paths[0]);
    expect(pickBranchPath(paths, 10)).toBe(paths[0]);
  });

  it('has none without paths', () => {
    expect(pickBranchPath([], 1)).toBeUndefined();
  });
});

describe('sampleAlong', () => {
  const straight: [number, number, number][] = [[0, 0, 0], [0, 1, 0], [0, 3, 0]];

  it('goes by the length of the path, not by its points', () => {
    expect(sampleAlong(straight, 0.5)!.position).toEqual([0, 1.5, 0]);
    expect(sampleAlong(straight, 0.25)!.position[1]).toBeCloseTo(0.75);
  });

  it('gives the direction of the segment', () => {
    expect(sampleAlong([[0, 0, 0], [2, 0, 0]], 0.5)!.tangent).toEqual([1, 0, 0]);
  });

  it('gives an orthogonal vector, along z for a vertical path', () => {
    expect(sampleAlong(straight, 0.5)!.orthogonal).toEqual([0, 0, -1]);
    const slanted = sampleAlong([[0, 0, 0], [1, 0, 0]], 0.5)!;
    // tangent cross up, so +z for a path along x
    expect(slanted.orthogonal[2]).toBeCloseTo(1);
  });

  it('clamps alpha to the ends', () => {
    expect(sampleAlong(straight, -1)!.position[1]).toBeCloseTo(0);
    expect(sampleAlong(straight, 2)!.position[1]).toBeCloseTo(3);
  });

  it('needs two points', () => {
    expect(sampleAlong([[0, 0, 0]], 0.5)).toBeUndefined();
  });
});

describe('angleStep', () => {
  it('is the short way around', () => {
    expect(angleStep(0, 0.5)).toBeCloseTo(0.5);
    expect(angleStep(3, -3)).toBeCloseTo(2 * Math.PI - 6);
    expect(angleStep(-3, 3)).toBeCloseTo(6 - 2 * Math.PI);
  });

  it('adds up to more than a full turn', () => {
    let last = 0;
    let turned = 0;
    for (let raw = 0.5; raw <= 8; raw += 0.5) {
      const wrapped = ((raw + Math.PI) % (2 * Math.PI)) - Math.PI;
      turned += angleStep(last, wrapped);
      last = wrapped;
    }
    expect(turned).toBeCloseTo(8, 5);
  });
});
