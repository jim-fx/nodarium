import { GEOMETRY_FLAGS, RESULT_TAGS } from '@nodarium/runtime';
import { isPath, pathPointCount, splitNestedArray } from '@nodarium/utils';
import {
  ArrowHelper,
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  Line,
  LineBasicMaterial,
  LineSegments,
  Matrix4,
  type Mesh,
  MeshBasicMaterial,
  Points,
  PointsMaterial,
  Vector3
} from 'three';
import { createGeometryPool } from '../geometry/geometryPool';
import { PATH_HEADER_SIZE } from '../gizmos/gizmo-paths';

// one color per stem depth, branches of branches get the next color
const depthColors = ['#ff5c5c', '#ffb347', '#4dd2ff', '#c084fc', '#5eea8c'].map((c) =>
  new Color(c)
);
const INSTANCE_COLOR = new Color('#5eea8c');
const POINT_COLOR = new Color('#f472b6');
// the arrow of a point is this many times the thickness of the path at the point
const POINT_ARROW_SCALE = 5;
// so a point on a very thin path still gets an arrow that can be seen
const POINT_ARROW_MIN = 0.05;
// instances are tiny next to the plant, their arrows are drawn this much longer
const INSTANCE_ARROW_SCALE = 100;
const RING_SEGMENTS = 12;
// drawn on top of the result
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

/** geometries as a wireframe */
function createGeometryDebug(geometries: Int32Array[]) {
  const group = new Group();
  const material = new MeshBasicMaterial({
    color: '#4dd2ff',
    wireframe: true,
    transparent: true,
    opacity: 0.35,
    depthTest: false
  });

  createGeometryPool(group, material).update(geometries);
  group.traverse((child) => {
    (child as Mesh).renderOrder = RENDER_ORDER;
  });

  return group;
}

/**
 * Instances as an arrow from their location along the direction they grow in. The geometry
 * grows along its -z axis. Instances are often tiny next to the plant, so the arrows are
 * scaled up by a fixed factor, they still grow and shrink with the size of the instances.
 */
function createInstanceDebug(instances: Int32Array[], amount = 1) {
  const group = new Group();
  const matrix = new Matrix4();
  const origin = new Vector3();
  const direction = new Vector3();

  for (const [index, data] of instances.entries()) {
    // 0: type, 1: vertices, 2: faces, 3: instances, 4: depth, followed by the faces,
    // the positions and normals of the geometry, then a matrix for every instance
    const vertices = data[1];
    const faces = data[2];
    const count = data[3];
    const offset = 5 + faces * 3 + vertices * 6;
    const f32 = new Float32Array(data.buffer, data.byteOffset, data.length);

    for (let i = 0; i < count; i++) {
      if (!isShown(itemKey(index, i), amount)) continue;
      matrix.fromArray(f32, offset + i * 16);
      origin.setFromMatrixPosition(matrix);
      direction.setFromMatrixColumn(matrix, 2).negate();
      const length = direction.length() * INSTANCE_ARROW_SCALE;
      if (!(length > 0)) continue;

      const arrow = new ArrowHelper(direction.normalize(), origin.clone(), length, INSTANCE_COLOR);
      (arrow.line.material as LineBasicMaterial).depthTest = false;
      (arrow.cone.material as MeshBasicMaterial).depthTest = false;
      arrow.line.renderOrder = RENDER_ORDER;
      arrow.cone.renderOrder = RENDER_ORDER;
      group.add(arrow);
    }
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

export function updateDebugScene(group: Group, data: Record<number, DebugEntry>) {
  clearGroup(group);
  for (const { type, data: value, amount, clip } of Object.values(data || {})) {
    // single values like floats and vectors have nothing to draw
    if (!DRAWN_TYPES.has(type) || !(value instanceof Int32Array)) continue;

    // a pattern only arrives as its preview geometry, which the output node wraps in a list.
    // One that could not be previewed is skipped.
    if (value[0] === PATTERN_TAG) continue;

    // a path output can carry geometry and instances along, e.g. from the instance node
    const items = splitItems(value);
    const paths = items.filter((item) => isPath(item));
    // a geometry with groups, like a mushroom, shows a share of its faces, the others by path
    const grouped = (item: Int32Array) => (item[4] & GEOMETRY_FLAGS.groups) !== 0;
    const geometries = items.filter((item) => item[0] === RESULT_TAGS.geometry && !grouped(item));
    const meshes = items.filter((item) => item[0] === RESULT_TAGS.geometry && grouped(item));
    const instances = items.filter((item) => item[0] === 2);
    const points = items.filter((item) => item[0] === 4);

    const shownPaths = pick(paths, idKeys(paths, (p) => p[2]), amount);
    const shownGeometries = pick(geometries, idKeys(geometries, (g) => g[3]), amount);
    const shownPoints = pick(points, points.map((p) => itemKey(p[1], p[3])), amount);

    if (shownPaths.length) group.add(createPathDebug(shownPaths, clip));
    if (shownGeometries.length) group.add(createGeometryDebug(shownGeometries));
    if (instances.length) group.add(createInstanceDebug(instances, amount));
    if (shownPoints.length) group.add(createPointsDebug(shownPoints));
    if (meshes.length) {
      group.add(createGeometryDebug(meshes.map((m) => selectFaces(m, amount))));
    }
  }
}
