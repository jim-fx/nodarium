import { collectTriangles } from '../render/triangles';

/** What a result looks like in a few numbers, to tell whether two builds made the same plant */
export type GeometrySummary = {
  triangles: number;
  min: number[];
  max: number[];
  /** the mean of all corners */
  center: number[];
  /** the mean distance of the corners from the center, along each axis */
  spread: number[];
};

export function geometrySummary(result: Int32Array): GeometrySummary {
  const { triangles } = collectTriangles(result);
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  const sum = [0, 0, 0];
  let count = 0;
  for (const t of triangles) {
    for (const v of [t.a, t.b, t.c]) {
      for (let d = 0; d < 3; d++) {
        min[d] = Math.min(min[d], v[d]);
        max[d] = Math.max(max[d], v[d]);
        sum[d] += v[d];
      }
      count++;
    }
  }
  const center = sum.map((s) => s / Math.max(1, count));
  const spread = [0, 0, 0];
  for (const t of triangles) {
    for (const v of [t.a, t.b, t.c]) {
      for (let d = 0; d < 3; d++) spread[d] += Math.abs(v[d] - center[d]);
    }
  }
  return {
    triangles: triangles.length,
    min,
    max,
    center,
    spread: spread.map((s) => s / Math.max(1, count))
  };
}
