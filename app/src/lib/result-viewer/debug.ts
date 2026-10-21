import { splitNestedArray } from '@nodarium/utils';
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
import { createGeometryPool } from './geometryPool';

// one color per stem depth, branches of branches get the next color
const depthColors = ['#ff5c5c', '#ffb347', '#4dd2ff', '#c084fc', '#5eea8c'].map((c) =>
  new Color(c)
);
const INSTANCE_COLOR = new Color('#5eea8c');
// instances are tiny next to the plant, their arrows are drawn this much longer
const INSTANCE_ARROW_SCALE = 100;
const RING_SEGMENTS = 12;
// drawn on top of the result
const RENDER_ORDER = 999;

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
  // 0: type, 1: depth, followed by x y z thickness per point
  for (let i = 2; i + 3 < f32.length; i += 4) {
    points.push([new Vector3(f32[i], f32[i + 1], f32[i + 2]), f32[i + 3]]);
  }
  return points;
}

/** the path as a line, a dot for every point and a ring showing its thickness */
function createPathDebug(paths: Int32Array[]) {
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
    const points = pathPoints(path);
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
function createInstanceDebug(instances: Int32Array[]) {
  const group = new Group();
  const matrix = new Matrix4();
  const origin = new Vector3();
  const direction = new Vector3();

  for (const data of instances) {
    // 0: type, 1: vertices, 2: faces, 3: instances, 4: depth, followed by the faces,
    // the positions and normals of the geometry, then a matrix for every instance
    const vertices = data[1];
    const faces = data[2];
    const count = data[3];
    const offset = 5 + faces * 3 + vertices * 6;
    const f32 = new Float32Array(data.buffer, data.byteOffset, data.length);

    for (let i = 0; i < count; i++) {
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

export function updateDebugScene(
  group: Group,
  data: Record<number, { type: string; data: Int32Array }>
) {
  clearGroup(group);
  for (const { type, data: value } of Object.values(data || {})) {
    // single values like floats and vectors have nothing to draw
    if ((type !== 'path' && type !== 'geometry') || !(value instanceof Int32Array)) continue;

    // a path output can carry geometry and instances along, e.g. from the instance node
    const items = splitItems(value);
    const paths = items.filter((item) => item[0] === 0);
    const geometries = items.filter((item) => item[0] === 1);
    const instances = items.filter((item) => item[0] === 2);

    if (paths.length) group.add(createPathDebug(paths));
    if (geometries.length) group.add(createGeometryDebug(geometries));
    if (instances.length) group.add(createInstanceDebug(instances));
  }
}
