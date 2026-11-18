import {
  NO_PATH,
  PATH_LINK_WORDS,
  readGeometryPathId,
  readInstancePathIds,
  splitNestedArray
} from '@nodarium/utils';

/** The first word of every item in a result says what it holds */
export const RESULT_TAGS = { geometry: 1, instances: 2, lines: 12 } as const;

/** Sorts the items of a result by their tag, items of other tags are left out */
export function sortResultItems(items: Int32Array[]) {
  const geometries: Int32Array[] = [];
  const instances: Int32Array[] = [];
  const lines: Int32Array[] = [];
  for (const item of items) {
    if (item[0] === RESULT_TAGS.geometry) geometries.push(item);
    else if (item[0] === RESULT_TAGS.instances) instances.push(item);
    else if (item[0] === RESULT_TAGS.lines) lines.push(item);
  }
  return { geometries, instances, lines };
}

/** Indexed triangles, all arrays are views into the data they were read from */
export type MeshData = {
  vertexCount: number;
  faceCount: number;
  /** three vertex indices per face */
  faces: Int32Array;
  /** x, y, z per vertex */
  positions: Float32Array;
  /** x, y, z per vertex */
  normals: Float32Array;
  /** RGBA8 per vertex in sRGB, red first, alpha 0 where a vertex has no color of its own */
  colors?: Uint8Array;
  /** material group per vertex, a geometry without them is one group */
  groups?: Int32Array;
  /** by group, the creator id of the graph node that made it, `NO_CREATOR` for none */
  creators?: Int32Array;
};

export type GeometryData = MeshData & {
  /** the id of the path the geometry was made from, `NO_PATH` (-1) for none */
  pathId: number;
  /**
   * the links of the paths it was made from, `PATH_LINK_WORDS` words each, see
   * `readPathLinkTable`. `to_geometry` writes them, so what sits on its paths leads to them.
   */
  paths?: Int32Array;
};

function readLayout(data: Int32Array, headerLength: number): MeshData {
  const vertexCount = data[1];
  const faceCount = data[2];
  const positionsStart = headerLength + faceCount * 3;
  const normalsStart = positionsStart + vertexCount * 3;
  const floats = (start: number) =>
    new Float32Array(data.buffer, data.byteOffset + start * 4, vertexCount * 3);
  return {
    vertexCount,
    faceCount,
    faces: data.subarray(headerLength, positionsStart),
    positions: floats(positionsStart),
    normals: floats(normalsStart)
  };
}

function colorsAt(data: Int32Array, start: number, vertexCount: number) {
  return new Uint8Array(data.buffer, data.byteOffset + start * 4, vertexCount * 4);
}

/** The bits of the flags word of a geometry, the fifth word of its header */
export const GEOMETRY_FLAGS = {
  /** one RGBA8 color per vertex follows the normals */
  colors: 1,
  /** the groups are followed by their number and the creator of every group */
  creators: 2,
  /** one group per vertex follows the colors, without them the geometry is one group */
  groups: 4,
  /**
   * the tables are followed by the number of paths the geometry was made from and their links,
   * `PATH_LINK_WORDS` words each, as in the header of a path
   */
  paths: 8
} as const;

/** The creator of a group that no graph node is known for */
export const NO_CREATOR = -(2 ** 31);

/**
 * Geometry: `[1, v, f, path id, flags, ...faces, ...positions, ...normals, ...colors?,
 * ...groups?, n?, ...creators?]`, the flags (`GEOMETRY_FLAGS`) say which of the optional
 * sections follow. The alpha of a color is how much of it shows over the material, 0 where a
 * vertex has no color of its own. With creators the groups are followed by their number and the
 * creator id of the graph node that made each of them.
 */
export function readGeometry(data: Int32Array): GeometryData {
  const geometry = readLayout(data, 5);
  const { vertexCount, faceCount } = geometry;
  const flags = data[4];
  const colorsStart = 5 + faceCount * 3 + vertexCount * 6;
  const groupsStart = colorsStart + (flags & GEOMETRY_FLAGS.colors ? vertexCount : 0);
  const groupsEnd = groupsStart + (flags & GEOMETRY_FLAGS.groups ? vertexCount : 0);
  const withCreators = (flags & GEOMETRY_FLAGS.creators) !== 0;
  const creatorsEnd = withCreators ? groupsEnd + 1 + Math.max(data[groupsEnd] ?? 0, 0) : groupsEnd;
  const withPaths = (flags & GEOMETRY_FLAGS.paths) !== 0;
  const expected = withPaths
    ? creatorsEnd + 1 + Math.max(data[creatorsEnd] ?? 0, 0) * PATH_LINK_WORDS
    : creatorsEnd;
  if (data.length !== expected) {
    throw new Error(
      `A geometry with ${vertexCount} vertices, ${faceCount} faces and flags ${flags} needs ${expected} words, got ${data.length}`
    );
  }
  if (flags & GEOMETRY_FLAGS.colors) geometry.colors = colorsAt(data, colorsStart, vertexCount);
  if (flags & GEOMETRY_FLAGS.groups) geometry.groups = data.subarray(groupsStart, groupsEnd);
  if (withCreators) geometry.creators = data.subarray(groupsEnd + 1, creatorsEnd);
  const paths = withPaths ? data.subarray(creatorsEnd + 1, expected) : undefined;
  return { ...geometry, pathId: readGeometryPathId(data), ...(paths && { paths }) };
}

export type InstancesData = MeshData & {
  instanceCount: number;
  /** how deep in the plant the path the instances sit on is */
  depth: number;
  /** a column-major 4x4 matrix per instance, the order of three.js `Matrix4.fromArray` */
  matrices: Float32Array;
  /** for every instance the id of the path it sits on, `NO_PATH` (-1) for none */
  pathIds: Int32Array;
};

/**
 * Plantarium instances: `[2, v, f, instances, depth, ...faces, ...positions, ...normals,
 * ...matrices, ...path ids, ...colors?]`, one shape drawn at every matrix. The colors of the
 * shape are there when the length leaves room for one per vertex after the path ids.
 */
export function readInstances(data: Int32Array): InstancesData {
  const shape = readLayout(data, 5);
  const instanceCount = data[3];
  const matricesStart = 5 + shape.faceCount * 3 + shape.vertexCount * 6;
  const colorsStart = matricesStart + instanceCount * 17;
  if (shape.vertexCount > 0 && data.length === colorsStart + shape.vertexCount) {
    shape.colors = colorsAt(data, colorsStart, shape.vertexCount);
  }
  const matrices = new Float32Array(
    data.buffer,
    data.byteOffset + matricesStart * 4,
    instanceCount * 16
  );
  let pathIds = readInstancePathIds(data);
  // older nodes wrote no path ids
  if (pathIds.length < instanceCount) {
    const padded = new Int32Array(instanceCount).fill(NO_PATH);
    padded.set(pathIds);
    pathIds = padded;
  }
  return { ...shape, instanceCount, depth: data[4], matrices, pathIds };
}

export type DecodedResult = {
  /**
   * Geometry: the tubes of a plant with a group per node that made them and the links of its
   * paths, the surface of a mushroom with its groups, geometry like a box as it is
   */
  geometries: GeometryData[];
  /** Plantarium instances like leaves, one shape and a matrix per instance */
  instances: InstancesData[];
  /** of everything, instances counted once per instance */
  vertexCount: number;
  /** of everything, instances counted once per instance */
  faceCount: number;
};

/**
 * Turns the result of `execute()` into typed arrays. They are views into `result`, copy them
 * before `result` is changed or transferred.
 */
export function decodeResult(result: Int32Array): DecodedResult {
  const sorted = sortResultItems(splitNestedArray(result));
  const geometries = sorted.geometries.map(readGeometry);
  const instances = sorted.instances.map(readInstances);

  let vertexCount = 0;
  let faceCount = 0;
  for (const item of geometries) {
    vertexCount += item.vertexCount;
    faceCount += item.faceCount;
  }
  for (const item of instances) {
    vertexCount += item.vertexCount * item.instanceCount;
    faceCount += item.faceCount * item.instanceCount;
  }
  return { geometries, instances, vertexCount, faceCount };
}
