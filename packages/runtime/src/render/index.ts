import { encodePng } from './png';
import { frameOf, rasterize, type View, VIEWS } from './rasterize';
import { cellOrigin, composeSheet, type SheetLayout } from './sheet';
import { bounds, collectTriangles, type Triangle, type Vec3 } from './triangles';

export type RenderView = View;

/** The views of a render without `views`: front, side and top */
export const DEFAULT_VIEWS: readonly RenderView[] = ['front', 'side', 'top'];

export const RENDER_VIEWS = Object.keys(VIEWS) as RenderView[];

export type RenderOptions = {
  /** the size of a single view in pixels, 512 by default */
  size?: number;
  /** adds a view from below after the others, for the underside of caps and leaves */
  below?: boolean;
  /** which views next to each other, in this order, {@link DEFAULT_VIEWS} by default */
  views?: readonly RenderView[];
  /**
   * Only what lies within `r` (m) of `x`, `z` seen from above, a close-up of one part. The size
   * is that of the close-up and the instances are not counted.
   */
  focus?: [x: number, z: number, r: number];
  /**
   * Also draws paths as tubes of their thickness and points as dots, for the
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

/** The views of `options`, `below` adds one from below when `views` doesn't have it */
function viewsOf({ views = DEFAULT_VIEWS, below }: RenderOptions): readonly RenderView[] {
  const unknown = views.find((view) => !(view in VIEWS));
  if (unknown !== undefined || views.length === 0) {
    throw new Error(`the views are ${RENDER_VIEWS.join(', ')}, not ${unknown ?? 'none'}`);
  }
  return below && !views.includes('below') ? [...views, 'below'] : views;
}

/** The triangles a render draws, null when there are none */
function drawnTriangles(result: Int32Array, options: RenderOptions) {
  const collected = collectTriangles(result, options.paths);
  let triangles = collected.triangles;
  if (options.focus) {
    const [x, z, r] = options.focus;
    triangles = triangles.filter(({ a, b, c }) =>
      Math.hypot((a[0] + b[0] + c[0]) / 3 - x, (a[2] + b[2] + c[2]) / 3 - z) <= r
    );
  }
  return triangles.length ? { triangles, instances: collected.instances } : null;
}

function statsOf(triangles: Triangle[], instances: number, options: RenderOptions): RenderStats {
  const { min, max } = bounds(triangles);
  // plants a few cm tall need more than two decimals
  const round = (v: number) => Math.round(v * 10000) / 10000;
  const leafArea = triangles.filter((t) => t.instanced).reduce(
    (sum, t) => sum + triangleArea(t),
    0
  );
  return {
    triangles: triangles.length,
    instances: options.focus ? undefined : instances,
    leafArea: Number(leafArea.toPrecision(4)),
    min: min.map(round) as RenderStats['min'],
    max: max.map(round) as RenderStats['max'],
    height: round(max[1] - Math.min(min[1], 0)),
    width: round(Math.max(max[0] - min[0], max[2] - min[2]))
  };
}

/**
 * Rasterizes a result in software: front, side and a view from above next to each other in one
 * PNG, or the `views` asked for. Stems are brown, instanced geometry (leaves, flowers) is green,
 * colors of plants and Myzel meshes show as they are. `result` is what `execute()` returns, or
 * the `data` of a `DebugEntry` with `paths: true`. Null when there is nothing to draw.
 */
export function renderToPng(
  result: Int32Array,
  options: RenderOptions = {}
): { png: Uint8Array; stats: RenderStats } | null {
  const views = viewsOf(options);
  const drawn = drawnTriangles(result, options);
  if (!drawn) return null;
  const png = encodePng(rasterize(drawn.triangles, options.size ?? 512, views));
  return { png, stats: statsOf(drawn.triangles, drawn.instances, options) };
}

export type SheetCell = {
  /** what `execute()` returned for this cell, an empty cell without it */
  result?: Int32Array;
  /** written into the top left corner: digits, ".", "-", "s", "t" and spaces */
  label?: string;
};

export type SheetOptions = RenderOptions & {
  /** the cells in a row, all in one row by default */
  columns?: number;
};

/** A cell of a sheet: where it is in pixels, and what it shows, null when it drew nothing */
export type PlacedCell = {
  column: number;
  row: number;
  x: number;
  y: number;
  stats: RenderStats | null;
};

export type RenderedSheet = {
  png: Uint8Array;
  width: number;
  height: number;
  columns: number;
  rows: number;
  /** the size of one cell, its views next to each other */
  cellWidth: number;
  cellHeight: number;
  cells: PlacedCell[];
};

/**
 * A contact sheet: every cell drawn like {@link renderToPng}, all at the same scale so seeds or
 * times of one graph can be compared, in rows of `columns` cells. Null when no cell has
 * anything to draw.
 */
export function renderSheetToPng(
  cells: SheetCell[],
  options: SheetOptions = {}
): RenderedSheet | null {
  const views = viewsOf(options);
  const size = options.size ?? 512;
  // the triangles of every cell twice, once for the frame and once to draw them, so only those
  // of one cell are held at a time
  const shown = cells.map((cell) => {
    const drawn = cell.result && drawnTriangles(cell.result, options);
    return drawn
      ? { ...bounds(drawn.triangles), stats: statsOf(drawn.triangles, drawn.instances, options) }
      : null;
  });
  const extents = shown.filter((cell) => cell !== null);
  if (extents.length === 0) return null;
  const min = [0, 1, 2].map((i) => Math.min(...extents.map((b) => b.min[i]))) as Vec3;
  const max = [0, 1, 2].map((i) => Math.max(...extents.map((b) => b.max[i]))) as Vec3;
  const frame = frameOf({ min, max });

  const columns = Math.max(1, Math.min(options.columns ?? cells.length, cells.length));
  const layout: SheetLayout = {
    columns,
    rows: Math.ceil(cells.length / columns),
    cellWidth: size * views.length,
    cellHeight: size,
    gap: Math.max(2, Math.round(size / 64))
  };
  const image = composeSheet(
    cells.map((cell, i) => {
      const drawn = shown[i] && drawnTriangles(cell.result!, options);
      const image = drawn ? rasterize(drawn.triangles, size, views, frame) : undefined;
      return { image, label: cell.label };
    }),
    layout
  );
  return {
    png: encodePng(image),
    width: image.width,
    height: image.height,
    columns: layout.columns,
    rows: layout.rows,
    cellWidth: layout.cellWidth,
    cellHeight: layout.cellHeight,
    cells: shown.map((cell, i) => ({ ...cellOrigin(layout, i), stats: cell?.stats ?? null }))
  };
}
