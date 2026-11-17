import { encodePng } from './png';
import { rasterize } from './rasterize';
import { bounds, collectTriangles, type Triangle } from './triangles';

export type RenderOptions = {
  /** the size of a single view in pixels, 512 by default */
  size?: number;
  /** adds a fourth view from below, for the underside of caps and leaves */
  below?: boolean;
  /**
   * Only what lies within `r` (m) of `x`, `z` seen from above, a close-up of one part. The size
   * is that of the close-up and the instances are not counted.
   */
  focus?: [x: number, z: number, r: number];
  /**
   * Also draws paths and Myzel lines as tubes of their thickness and points as dots, for the
   * output of a node in the middle of a graph. The result of a graph carries its paths next to
   * their geometry, so it is off by default.
   */
  paths?: boolean;
};

/** What a render shows, lengths in meters */
export type RenderStats = {
  triangles: number;
  /** left out with a focus */
  instances?: number;
  /** the area of the instanced triangles, the leaves of a plant, in m² */
  leafArea: number;
  min: [number, number, number];
  max: [number, number, number];
  /** from the ground or the lowest point, whichever is lower, to the top */
  height: number;
  /** the larger of the extents along x and z */
  width: number;
};

function triangleArea({ a, b, c }: Triangle) {
  const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
  return Math.hypot(
    u[1] * v[2] - u[2] * v[1],
    u[2] * v[0] - u[0] * v[2],
    u[0] * v[1] - u[1] * v[0]
  ) / 2;
}

/**
 * Rasterizes a result in software: front, side and a view from above next to each other in one
 * PNG. Stems are brown, instanced geometry (leaves, flowers) is green, colors of plants and
 * Myzel meshes show as they are. `result` is what `execute()` returns, or the `data` of a
 * `DebugEntry` with `paths: true`. Null when there is nothing to draw.
 */
export function renderToPng(
  result: Int32Array,
  options: RenderOptions = {}
): { png: Uint8Array; stats: RenderStats } | null {
  const collected = collectTriangles(result, options.paths);
  let triangles = collected.triangles;
  if (options.focus) {
    const [x, z, r] = options.focus;
    triangles = triangles.filter(({ a, b, c }) =>
      Math.hypot((a[0] + b[0] + c[0]) / 3 - x, (a[2] + b[2] + c[2]) / 3 - z) <= r
    );
  }
  if (triangles.length === 0) return null;

  const png = encodePng(rasterize(triangles, options.size ?? 512, options.below ?? false));

  const { min, max } = bounds(triangles);
  // plants a few cm tall need more than two decimals
  const round = (v: number) => Math.round(v * 10000) / 10000;
  const leafArea = triangles.filter((t) => t.instanced).reduce(
    (sum, t) => sum + triangleArea(t),
    0
  );
  return {
    png,
    stats: {
      triangles: triangles.length,
      instances: options.focus ? undefined : collected.instances,
      leafArea: Number(leafArea.toPrecision(4)),
      min: min.map(round) as RenderStats['min'],
      max: max.map(round) as RenderStats['max'],
      height: round(max[1] - Math.min(min[1], 0)),
      width: round(Math.max(max[0] - min[0], max[2] - min[2]))
    }
  };
}
