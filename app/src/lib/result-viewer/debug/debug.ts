import { GEOMETRY_FLAGS, RESULT_TAGS } from '@nodarium/runtime';
import { isPath, pathPointCount, splitNestedArray } from '@nodarium/utils';
import {
  ArrowHelper,
  Box3,
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  Line,
  LineBasicMaterial,
  LineSegments,
  type Material,
  type MeshBasicMaterial,
  Points,
  PointsMaterial,
  ShaderMaterial,
  Vector3
} from 'three';
import { createResultPools, takeVertexColors } from '../geometry/geometryPool';
import { PATH_HEADER_SIZE } from '../gizmos/gizmo-paths';

// one color per stem depth, branches of branches get the next color
const depthColors = ['#ff5c5c', '#ffb347', '#4dd2ff', '#c084fc', '#5eea8c'].map((c) =>
  new Color(c)
);
const POINT_COLOR = new Color('#f472b6');
// the arrow of a point is this many times the thickness of the path at the point
const POINT_ARROW_SCALE = 5;
// so a point on a very thin path still gets an arrow that can be seen
const POINT_ARROW_MIN = 0.05;
const RING_SEGMENTS = 12;
// the lines are drawn on top of the result, paths run inside their tubes
const RENDER_ORDER = 999;

const DRAWN_TYPES = new Set(['path', 'geometry', 'points', 'pattern']);
// a 2D pattern, see nodarium_utils::pattern
const PATTERN_TAG = 15;

export type DebugEntry = {
  type: string;
  data: Int32Array;
  /** the share of the items that is drawn, 0–1 */
  amount?: number;
  /** the share of the length of each path that is drawn, 0–1 */
  clip?: number;
};

/** lowbias32 */
function hash(x: number) {
  x ^= x >>> 16;
  x = Math.imul(x, 0x7feb352d);
  x ^= x >>> 15;
  x = Math.imul(x, 0x846ca68b);
  x ^= x >>> 16;
  return x >>> 0;
}

/** A key made of several numbers, e.g. the index of an instance and the item it is in */
function itemKey(...parts: number[]) {
  return parts.reduce((key, part) => hash(key ^ part), 0x9e3779b9);
}

/**
 * Whether the item with this key is drawn. Every key has a fixed threshold, so the same items
 * stay while the graph runs again and a larger amount only adds items.
 */
export function isShown(key: number, amount = 1) {
  return amount >= 1 || hash(key) / 2 ** 32 < amount;
}

/** Keys for items that carry an id, items with the same id are told apart by their order */
function idKeys(items: Int32Array[], idOf: (item: Int32Array) => number) {
  const seen = new Map<number, number>();
  return items.map((item) => {
    const id = idOf(item);
    const n = seen.get(id) ?? 0;
    seen.set(id, n + 1);
    return itemKey(id, n);
  });
}

function pick(items: Int32Array[], keys: number[], amount = 1) {
  return amount >= 1 ? items : items.filter((_, i) => isShown(keys[i], amount));
}

/** The start of a path up to `clip` of its length, the last point lies on the cut */
export function clipPoints(points: [Vector3, number][], clip = 1) {
  if (clip >= 1) return points;
  if (!(clip > 0) || !points.length) return [];
  let total = 0;
  for (let i = 1; i < points.length; i++) total += points[i][0].distanceTo(points[i - 1][0]);
  const cut = total * clip;
  const clipped: [Vector3, number][] = [points[0]];
  let length = 0;
  for (let i = 1; i < points.length; i++) {
    const [a, ta] = points[i - 1];
    const [b, tb] = points[i];
    const step = a.distanceTo(b);
    if (length + step >= cut) {
      const t = step > 0 ? (cut - length) / step : 0;
      clipped.push([a.clone().lerp(b, t), ta + (tb - ta) * t]);
      break;
    }
    clipped.push(points[i]);
    length += step;
  }
  return clipped;
}

/** The words of a geometry before its faces: type, vertices, faces, path id and flags */
const GEOMETRY_HEADER = 5;

/** A geometry with groups with `amount` of its faces, its vertices stay as they are */
function selectFaces(data: Int32Array, amount = 1) {
  if (amount >= 1) return data;
  const faces = data[2];
  const kept: number[] = [];
  for (let f = 0; f < faces; f++) {
    const at = GEOMETRY_HEADER + f * 3;
    if (isShown(itemKey(f), amount)) kept.push(...data.subarray(at, at + 3));
  }
  const result = new Int32Array(data.length - (faces * 3 - kept.length));
  result.set(data.subarray(0, GEOMETRY_HEADER));
  result[2] = kept.length / 3;
  result.set(kept, GEOMETRY_HEADER);
  result.set(data.subarray(GEOMETRY_HEADER + faces * 3), GEOMETRY_HEADER + kept.length);
  return result;
}

function depthColor(depth: number) {
  return depthColors[Math.max(depth - 1, 0) % depthColors.length];
}

/** a single encoded item is not wrapped in an outer list, so splitNestedArray finds nothing */
function splitItems(data: Int32Array) {
  const items = splitNestedArray(data);
  if (items.length === 0 && data[0] === 0 && data.length > 4) {
    return [data.slice(2, data.length - 2)];
  }
  return items;
}

function pathPoints(data: Int32Array) {
  const f32 = new Float32Array(data.buffer, data.byteOffset, data.length);
  const points: [Vector3, number][] = [];
  // the header is followed by x y z thickness per point, and by the colors of a colored path
  for (let i = PATH_HEADER_SIZE; i < PATH_HEADER_SIZE + pathPointCount(data) * 4; i += 4) {
    points.push([new Vector3(f32[i], f32[i + 1], f32[i + 2]), f32[i + 3]]);
  }
  return points;
}

/** the path as a line, a dot for every point and a ring showing its thickness */
function createPathDebug(paths: Int32Array[], clip = 1) {
  const group = new Group();
  const dots: number[] = [];
  const dotColors: number[] = [];
  const rings: number[] = [];
  const ringColors: number[] = [];

  const tangent = new Vector3();
  const side = new Vector3();
  const up = new Vector3();
  const a = new Vector3();
  const b = new Vector3();

  for (const path of paths) {
    const points = clipPoints(pathPoints(path), clip);
    if (!points.length) continue;
    const color = depthColor(path[1]);

    if (points.length >= 2) {
      const line = new Line(
        new BufferGeometry().setFromPoints(points.map(([p]) => p)),
        new LineBasicMaterial({ color, depthTest: false })
      );
      line.renderOrder = RENDER_ORDER;
      group.add(line);
    }

    for (let i = 0; i < points.length; i++) {
      const [p, thickness] = points[i];
      dots.push(p.x, p.y, p.z);
      dotColors.push(color.r, color.g, color.b);

      if (!(thickness > 0) || points.length < 2) continue;
      const prev = points[Math.max(i - 1, 0)][0];
      const next = points[Math.min(i + 1, points.length - 1)][0];
      tangent.subVectors(next, prev).normalize();
      // any vector that is not parallel to the tangent works as a starting point
      side.set(Math.abs(tangent.y) < 0.9 ? 0 : 1, Math.abs(tangent.y) < 0.9 ? 1 : 0, 0)
        .cross(tangent).normalize().multiplyScalar(thickness);
      up.crossVectors(tangent, side);
      for (let s = 0; s < RING_SEGMENTS; s++) {
        const t0 = (s / RING_SEGMENTS) * Math.PI * 2;
        const t1 = ((s + 1) / RING_SEGMENTS) * Math.PI * 2;
        a.copy(p).addScaledVector(side, Math.cos(t0)).addScaledVector(up, Math.sin(t0));
        b.copy(p).addScaledVector(side, Math.cos(t1)).addScaledVector(up, Math.sin(t1));
        rings.push(a.x, a.y, a.z, b.x, b.y, b.z);
        ringColors.push(color.r, color.g, color.b, color.r, color.g, color.b);
      }
    }
  }

  if (rings.length) {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new Float32BufferAttribute(rings, 3));
    geometry.setAttribute('color', new Float32BufferAttribute(ringColors, 3));
    const ringLines = new LineSegments(
      geometry,
      new LineBasicMaterial({
        vertexColors: true,
        transparent: true,
        opacity: 0.4,
        depthTest: false
      })
    );
    ringLines.renderOrder = RENDER_ORDER;
    group.add(ringLines);
  }

  if (dots.length) {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new Float32BufferAttribute(dots, 3));
    geometry.setAttribute('color', new Float32BufferAttribute(dotColors, 3));
    const points = new Points(
      geometry,
      new PointsMaterial({ size: 5, sizeAttenuation: false, vertexColors: true, depthTest: false })
    );
    points.renderOrder = RENDER_ORDER;
    group.add(points);
  }

  return group;
}

/**
 * Points as a dot with an arrow along their direction, the way a branch on them would grow,
 * and a short line along the tangent of the path they sit on.
 */
function createPointsDebug(points: Int32Array[]) {
  const group = new Group();
  const dots: number[] = [];
  const tangents: number[] = [];
  const position = new Vector3();
  const direction = new Vector3();
  const tangent = new Vector3();

  for (const data of points) {
    // 0: type, 1: parent id, 2: depth, 3: index, followed by the position, the direction,
    // the tangent, the thickness of the path and where on it the point sits
    const f32 = new Float32Array(data.buffer, data.byteOffset, data.length);
    position.fromArray(f32, 4);
    direction.fromArray(f32, 7);
    tangent.fromArray(f32, 10);
    const length = Math.max(f32[13], POINT_ARROW_MIN) * POINT_ARROW_SCALE;
    if (!position.toArray().every(Number.isFinite)) continue;

    dots.push(position.x, position.y, position.z);
    if (tangent.lengthSq() > 0) {
      tangent.normalize().multiplyScalar(length / 2);
      tangents.push(
        position.x - tangent.x,
        position.y - tangent.y,
        position.z - tangent.z,
        position.x + tangent.x,
        position.y + tangent.y,
        position.z + tangent.z
      );
    }
    if (!(direction.lengthSq() > 0)) continue;

    const arrow = new ArrowHelper(direction.normalize(), position.clone(), length, POINT_COLOR);
    (arrow.line.material as LineBasicMaterial).depthTest = false;
    (arrow.cone.material as MeshBasicMaterial).depthTest = false;
    arrow.line.renderOrder = RENDER_ORDER;
    arrow.cone.renderOrder = RENDER_ORDER;
    group.add(arrow);
  }

  if (tangents.length) {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new Float32BufferAttribute(tangents, 3));
    const lines = new LineSegments(
      geometry,
      new LineBasicMaterial({
        color: POINT_COLOR,
        transparent: true,
        opacity: 0.5,
        depthTest: false
      })
    );
    lines.renderOrder = RENDER_ORDER;
    group.add(lines);
  }

  if (dots.length) {
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new Float32BufferAttribute(dots, 3));
    const dotPoints = new Points(
      geometry,
      new PointsMaterial({ size: 8, sizeAttenuation: false, color: POINT_COLOR, depthTest: false })
    );
    dotPoints.renderOrder = RENDER_ORDER;
    group.add(dotPoints);
  }

  return group;
}

/** The words of an instance item before its faces: type, vertices, faces, instances, creator */
const INSTANCES_HEADER = 5;

/** The instances of an item with `amount` of them, the geometry they share stays as it is */
function selectInstances(data: Int32Array, index: number, amount = 1) {
  if (amount >= 1) return data;
  const count = data[3];
  const matrices = INSTANCES_HEADER + data[2] * 3 + data[1] * 6;
  const ids = matrices + count * 16;
  const hasIds = data.length >= ids + count;
  const kept: number[] = [];
  for (let i = 0; i < count; i++) if (isShown(itemKey(index, i), amount)) kept.push(i);
  const colors = data.subarray(hasIds ? ids + count : ids);
  const result = new Int32Array(
    matrices + kept.length * (hasIds ? 17 : 16) + colors.length
  );
  result.set(data.subarray(0, matrices));
  result[3] = kept.length;
  let at = matrices;
  for (const i of kept) {
    result.set(data.subarray(matrices + i * 16, matrices + i * 16 + 16), at);
    at += 16;
  }
  if (hasIds) { for (const i of kept) result[at++] = data[ids + i]; }
  result.set(colors, at);
  return result;
}

/** What one debug entry draws: surfaces like the result, paths and points as lines */
function debugItems({ type, data, amount }: DebugEntry) {
  // single values like floats and vectors have nothing to draw. A pattern only arrives as its
  // preview geometry, which the output node wraps in a list, one that could not be previewed is
  // skipped.
  if (!DRAWN_TYPES.has(type) || !(data instanceof Int32Array) || data[0] === PATTERN_TAG) return;

  // a path output can carry geometry and instances along, e.g. from the instance node
  const items = splitItems(data);
  const paths = items.filter((item) => isPath(item));
  // a geometry with groups, like a mushroom, shows a share of its faces, the others by path
  const grouped = (item: Int32Array) => (item[4] & GEOMETRY_FLAGS.groups) !== 0;
  const geometries = items.filter((item) => item[0] === RESULT_TAGS.geometry && !grouped(item));
  const meshes = items.filter((item) => item[0] === RESULT_TAGS.geometry && grouped(item));
  const instances = items.filter((item) => item[0] === RESULT_TAGS.instances);
  const points = items.filter((item) => item[0] === 4);

  return {
    paths: pick(paths, idKeys(paths, (p) => p[2]), amount),
    points: pick(points, points.map((p) => itemKey(p[1], p[3])), amount),
    surfaces: [
      ...pick(geometries, idKeys(geometries, (g) => g[3]), amount),
      ...meshes.map((m) => selectFaces(m, amount)),
      ...instances.map((data, index) => selectInstances(data, index, amount))
    ]
  };
}

/** Whether `entry` has anything to draw, a float or a vector has not */
export function drawsSomething(entry: DebugEntry) {
  const items = debugItems({ ...entry, amount: 1 });
  return !!items && (items.paths.length + items.points.length + items.surfaces.length) > 0;
}

const inFrontMaterials = new WeakMap<Material, Material>();

/**
 * `material` drawn in front of a surface at the same place. The debug node often shows a surface
 * the result has as well, the dimmed copy of the result would fight with it. The colours of a
 * geometry, like the disc of a pattern, are what the debug node is there to show, so every
 * material with a surface colour shows them, not only the one of the plants.
 */
export function inFront(material: Material) {
  let front = inFrontMaterials.get(material);
  if (front) return front;
  front = material.clone();
  front.polygonOffset = true;
  front.polygonOffsetFactor = -1;
  front.polygonOffsetUnits = -4;
  // the height range is set on the uniforms of the material itself
  if (front instanceof ShaderMaterial && material instanceof ShaderMaterial) {
    front.uniforms = material.uniforms;
  } else {
    takeVertexColors(front);
  }
  front.onBeforeCompile = (shader, renderer) => material.onBeforeCompile(shader, renderer);
  front.customProgramCacheKey = () => material.customProgramCacheKey();
  inFrontMaterials.set(material, front);
  return front;
}

function clearGroup(group: Group) {
  group.traverse((child) => {
    if (child === group) return;
    // @ts-expect-error three.js runtime fields
    child.geometry?.dispose?.();
    // @ts-expect-error three.js runtime fields
    child.material?.dispose?.();
  });
  group.clear();
}

/**
 * Draws what one debug node shows. Geometry, geometry with groups, the disc of a pattern and
 * instances are drawn like the result, with the material of the viewer and its colours. Paths
 * and points have no surface and stay lines on top of everything.
 */
export function createDebugScene(parent: Group, material: Material) {
  const surfaces = new Group();
  const lines = new Group();
  parent.add(surfaces, lines);
  let pools: ReturnType<typeof createResultPools> | undefined;
  let shown = material;

  return {
    setMaterial(next: Material) {
      shown = next;
      pools?.setMaterial(inFront(next));
    },
    /**
     * Shows `entry` instead of what was shown, nothing without one. Returns the bounds of what is
     * drawn, empty when there is nothing to draw.
     */
    update(entry: DebugEntry | undefined) {
      clearGroup(lines);
      const items = entry && debugItems(entry);
      const bounds = new Box3();

      if (items?.surfaces.length || pools) {
        pools ??= createResultPools(surfaces, inFront(shown));
        bounds.union(pools.update(items?.surfaces ?? []).bounds);
      }
      if (items?.paths.length) lines.add(createPathDebug(items.paths, entry?.clip));
      if (items?.points.length) lines.add(createPointsDebug(items.points));
      if (lines.children.length) bounds.union(new Box3().setFromObject(lines));
      return bounds;
    }
  };
}
