import { splitNestedArray } from '@nodarium/utils';
import type { Vec3 } from './gizmos';

/** type, depth, id, parent id and where on the parent the path is attached */
export const PATH_HEADER_SIZE = 5;

export type ProbePath = { depth: number; points: Vec3[] };

/** The paths in the output of a node, with the points of every path */
export function readPaths(data: Int32Array | undefined): ProbePath[] {
  if (!data?.length) return [];

  // a single item is not wrapped in a list, so there is nothing to split
  let items = splitNestedArray(data);
  if (items.length === 0 && data[0] === 0 && data.length > 4) {
    items = [data.slice(2, data.length - 2)];
  }

  return items
    .filter((item) => item[0] === 0 && item.length >= PATH_HEADER_SIZE)
    .map((item) => {
      const f32 = new Float32Array(item.buffer, item.byteOffset, item.length);
      const points: Vec3[] = [];
      for (let i = PATH_HEADER_SIZE; i + 3 < f32.length; i += 4) {
        points.push([f32[i], f32[i + 1], f32[i + 2]]);
      }
      return { depth: item[1], points };
    });
}

/** The path the branch node works on first, the same layer it picks: the outermost `depth` ones */
export function pickBranchPath(paths: ProbePath[], depth: number) {
  if (!paths.length) return;
  const maxDepth = Math.max(...paths.map((p) => p.depth));
  return paths.find((p) => p.depth >= maxDepth - depth + 1);
}

export type PathSample = { position: Vec3; tangent: Vec3; orthogonal: Vec3 };

function sub(a: Vec3, b: Vec3): Vec3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

function cross(a: Vec3, b: Vec3): Vec3 {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

function normalize(a: Vec3): Vec3 | undefined {
  const length = Math.hypot(...a);
  return length > 0 ? [a[0] / length, a[1] / length, a[2] / length] : undefined;
}

/**
 * A spot along a path by its alpha (0 to 1 along its length) with the direction of the path and a
 * vector orthogonal to it. This is what `interpolate_along_path` gives the branch node, so the
 * gizmo shows the branch where the node puts it.
 */
export function sampleAlong(points: Vec3[], alpha: number): PathSample | undefined {
  if (points.length < 2) return;
  const a = Math.min(Math.max(alpha, 0.000001), 0.999999);

  const lengths = points.slice(1).map((p, i) => Math.hypot(...sub(p, points[i])));
  const total = lengths.reduce((sum, l) => sum + l, 0);
  const target = a * total;

  let accumulated = 0;
  let segment = lengths.length - 1;
  let t = 1;
  for (let i = 0; i < lengths.length; i++) {
    if (accumulated + lengths[i] >= target) {
      segment = i;
      t = lengths[i] > 0 ? (target - accumulated) / lengths[i] : 0;
      break;
    }
    accumulated += lengths[i];
  }

  const start = points[segment];
  const end = points[segment + 1];
  const tangent = normalize(sub(end, start)) ?? [0, 1, 0];
  // a vertical segment has no orthogonal in the horizontal plane, it gets one along the z axis
  const orthogonal = normalize(cross(tangent, [0, 1, 0])) ?? normalize(cross(tangent, [1, 0, 0]))!;

  return {
    position: [
      start[0] + (end[0] - start[0]) * t,
      start[1] + (end[1] - start[1]) * t,
      start[2] + (end[2] - start[2]) * t
    ],
    tangent,
    orthogonal
  };
}

/** The signed difference between two angles, the short way around */
export function angleStep(previous: number, next: number) {
  const full = Math.PI * 2;
  let step = (next - previous) % full;
  if (step > Math.PI) step -= full;
  if (step <= -Math.PI) step += full;
  return step;
}
