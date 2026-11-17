import { isPath, PATH_HEADER_SIZE, pathPointCount, splitNestedArray } from '@nodarium/utils';
import { RESULT_TAGS } from '../result/decode-result';

export type Vec3 = [number, number, number];
/** `color` is the mean sRGB color of its vertices, over the default color by their alpha */
export type Triangle = { a: Vec3; b: Vec3; c: Vec3; instanced: boolean; color?: Vec3 };

export const STEM_COLOR: Vec3 = [125, 95, 60];
export const INSTANCE_COLOR: Vec3 = [70, 140, 60];
const POINT_COLOR: Vec3 = [214, 84, 150];

const POINTS_TAG = 4;
const LINE_WORDS = 7;
const TUBE_SIDES = 6;
/** paths thinner than this share of the size of everything drawn are drawn this thick */
const MIN_LINE_RADIUS = 0.002;
const MIN_POINT_RADIUS = 0.008;

function floats(data: Int32Array, offset: number, length: number) {
  return new Float32Array(data.buffer, data.byteOffset + offset * 4, length);
}

/** The mean sRGB color of the corners of a face, over `fallback` by their alpha */
function blend(colors: Uint8Array | undefined, corners: number[], fallback: number[]) {
  if (!colors) return undefined;
  return [0, 1, 2].map((c) =>
    corners.reduce((sum, v) => {
      const alpha = colors[v * 4 + 3] / 255;
      return sum + (colors[v * 4 + c] * alpha + fallback[c] * (1 - alpha)) / 3;
    }, 0)
  ) as Vec3;
}

function addMesh(part: Int32Array, triangles: Triangle[]) {
  // [14, vertices, faces, flags, ...faces, ...positions, ...normals, ...colors?, ...groups]
  const [vertexCount, faceCount] = [part[1], part[2]];
  const faces = part.subarray(4, 4 + faceCount * 3);
  const positions = floats(part, 4 + faceCount * 3, vertexCount * 3);
  const vertex = (
    v: number
  ): Vec3 => [positions[v * 3], positions[v * 3 + 1], positions[v * 3 + 2]];
  const colorStart = 4 + faceCount * 3 + vertexCount * 6;
  const colors = part[3] & 1
    ? new Uint8Array(part.buffer, part.byteOffset + colorStart * 4, vertexCount * 4)
    : undefined;
  for (let f = 0; f < faceCount; f++) {
    const corners = [faces[f * 3], faces[f * 3 + 1], faces[f * 3 + 2]];
    triangles.push({
      a: vertex(corners[0]),
      b: vertex(corners[1]),
      c: vertex(corners[2]),
      instanced: false,
      color: blend(colors, corners, STEM_COLOR)
    });
  }
}

/** Plantarium geometry and instances, returns the number of instances */
function addGeometry(part: Int32Array, triangles: Triangle[]) {
  // geometry: [1, vertices, faces, path id, ...faces, ...positions, ...normals, ...colors?]
  // instances: [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals,
  //   ...matrices, ...path ids, ...colors?]
  const instanced = part[0] === RESULT_TAGS.instances;
  let index = 1;
  const vertexCount = part[index++];
  const faceCount = part[index++];
  const instanceCount = instanced ? part[index++] : 1;
  // the depth of instances, the path id of geometry
  index++;
  const faces = part.subarray(index, index + faceCount * 3);
  index += faceCount * 3;
  const positions = floats(part, index, vertexCount * 3);
  index += vertexCount * 6;
  const matrices = instanced ? floats(part, index, instanceCount * 16) : undefined;
  const colorStart = index + (instanced ? instanceCount * 17 : 0);
  const colors = vertexCount > 0 && part.length === colorStart + vertexCount
    ? new Uint8Array(part.buffer, part.byteOffset + colorStart * 4, vertexCount * 4)
    : undefined;
  const fallback = instanced ? INSTANCE_COLOR : STEM_COLOR;

  for (let i = 0; i < instanceCount; i++) {
    const m = matrices?.subarray(i * 16, i * 16 + 16);
    const vertex = (v: number): Vec3 => {
      const x = positions[v * 3], y = positions[v * 3 + 1], z = positions[v * 3 + 2];
      if (!m) return [x, y, z];
      return [
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14]
      ];
    };
    for (let f = 0; f < faceCount; f++) {
      const corners = [faces[f * 3], faces[f * 3 + 1], faces[f * 3 + 2]];
      triangles.push({
        a: vertex(corners[0]),
        b: vertex(corners[1]),
        c: vertex(corners[2]),
        instanced,
        color: blend(colors, corners, fallback)
      });
    }
  }
  return instanced ? instanceCount : 0;
}

type Segment = { a: Vec3; b: Vec3; ra: number; rb: number };
type Dot = { p: Vec3; r: number };

function pathSegments(path: Int32Array, segments: Segment[]) {
  const f = floats(path, 0, path.length);
  const count = pathPointCount(path);
  for (let i = 0; i + 1 < count; i++) {
    const o = PATH_HEADER_SIZE + i * 4;
    segments.push({
      a: [f[o], f[o + 1], f[o + 2]],
      b: [f[o + 4], f[o + 5], f[o + 6]],
      ra: f[o + 3],
      rb: f[o + 7]
    });
  }
}

function lineSegments(lines: Int32Array, segments: Segment[]) {
  // [12, count, (x0 y0 z0 x1 y1 z1 group)...]
  const f = floats(lines, 0, lines.length);
  for (let i = 0; i < lines[1]; i++) {
    const o = 2 + i * LINE_WORDS;
    segments.push({
      a: [f[o], f[o + 1], f[o + 2]],
      b: [f[o + 3], f[o + 4], f[o + 5]],
      ra: 0,
      rb: 0
    });
  }
}

function pointDot(point: Int32Array): Dot {
  // [4, parent id, depth, index, position, direction, tangent, thickness, alpha]
  const f = floats(point, 0, point.length);
  return { p: [f[4], f[5], f[6]], r: f[13] };
}

function perpendicular(d: Vec3): [Vec3, Vec3] {
  const helper: Vec3 = Math.abs(d[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
  const cross = (a: Vec3, b: Vec3): Vec3 => [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0]
  ];
  const u = cross(d, helper);
  const length = Math.hypot(...u);
  const un = u.map((v) => v / length) as Vec3;
  return [un, cross(d, un)];
}

function addTube({ a, b, ra, rb }: Segment, triangles: Triangle[]) {
  const d: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const length = Math.hypot(...d);
  if (!(length > 0)) return;
  const [u, v] = perpendicular(d.map((x) => x / length) as Vec3);
  const ring = (c: Vec3, r: number) =>
    Array.from({ length: TUBE_SIDES }, (_, i): Vec3 => {
      const t = (i / TUBE_SIDES) * Math.PI * 2;
      const cos = Math.cos(t) * r, sin = Math.sin(t) * r;
      return [
        c[0] + u[0] * cos + v[0] * sin,
        c[1] + u[1] * cos + v[1] * sin,
        c[2] + u[2] * cos + v[2] * sin
      ];
    });
  const ringA = ring(a, ra), ringB = ring(b, rb);
  for (let i = 0; i < TUBE_SIDES; i++) {
    const j = (i + 1) % TUBE_SIDES;
    triangles.push({ a: ringA[i], b: ringB[i], c: ringB[j], instanced: false });
    triangles.push({ a: ringA[i], b: ringB[j], c: ringA[j], instanced: false });
  }
}

function addDot({ p, r }: Dot, triangles: Triangle[]) {
  const corners: Vec3[] = [
    [r, 0, 0],
    [-r, 0, 0],
    [0, r, 0],
    [0, -r, 0],
    [0, 0, r],
    [0, 0, -r]
  ].map(([x, y, z]) => [p[0] + x, p[1] + y, p[2] + z]);
  for (const x of [0, 1]) {
    for (const y of [2, 3]) {
      for (const z of [4, 5]) {
        triangles.push({
          a: corners[x],
          b: corners[y],
          c: corners[z],
          instanced: false,
          color: POINT_COLOR
        });
      }
    }
  }
}

/** The items of a value: a list, or a single mesh or lines as a node gives them on their own */
function itemsOf(data: Int32Array) {
  if (data[0] === RESULT_TAGS.mesh || data[0] === RESULT_TAGS.lines) return [data];
  const items = splitNestedArray(data);
  // a single encoded item is not wrapped in an outer list
  if (items.length === 0 && data[0] === 0 && data.length > 4) return [data.slice(2, -2)];
  return items;
}

/**
 * The triangles of the geometry, the instances and the Myzel meshes in a result. With `paths`
 * also paths and Myzel lines as tubes of their thickness and points as small dots, for the
 * output of a node in the middle of a graph.
 */
export function collectTriangles(result: Int32Array, paths = false) {
  const triangles: Triangle[] = [];
  const segments: Segment[] = [];
  const dots: Dot[] = [];
  let instances = 0;

  for (const part of paths ? itemsOf(result) : splitNestedArray(result)) {
    if (part[0] === RESULT_TAGS.mesh) addMesh(part, triangles);
    else if (part[0] === RESULT_TAGS.geometry || part[0] === RESULT_TAGS.instances) {
      instances += addGeometry(part, triangles);
    } else if (!paths) continue;
    else if (isPath(part)) pathSegments(part, segments);
    else if (part[0] === RESULT_TAGS.lines) lineSegments(part, segments);
    else if (part[0] === POINTS_TAG) dots.push(pointDot(part));
  }

  if (segments.length || dots.length) {
    const positions = [...segments.flatMap((s) => [s.a, s.b]), ...dots.map((d) => d.p)];
    const { min, max } = bounds(positions.map((p) => ({ a: p, b: p, c: p, instanced: false })));
    const extent = Math.max(max[0] - min[0], max[1] - min[1], max[2] - min[2], 1e-6);
    for (const s of segments) {
      const r = extent * MIN_LINE_RADIUS;
      addTube({ ...s, ra: Math.max(s.ra, r), rb: Math.max(s.rb, r) }, triangles);
    }
    for (const d of dots) {
      addDot({ p: d.p, r: Math.max(d.r, extent * MIN_POINT_RADIUS) }, triangles);
    }
  }

  return { triangles, instances };
}

export function bounds(triangles: Triangle[]) {
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const t of triangles) {
    for (const v of [t.a, t.b, t.c]) {
      for (let d = 0; d < 3; d++) {
        min[d] = Math.min(min[d], v[d]);
        max[d] = Math.max(max[d], v[d]);
      }
    }
  }
  return { min, max };
}
