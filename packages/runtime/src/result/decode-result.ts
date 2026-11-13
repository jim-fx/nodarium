import {
  NO_PATH,
  readGeometryPathId,
  readInstancePathIds,
  splitNestedArray
} from '@nodarium/utils';

/** The first word of every item in a result says what it holds */
export const RESULT_TAGS = { geometry: 1, instances: 2, lines: 12, mesh: 14 } as const;

/** Sorts the items of a result by their tag, items of other tags are left out */
export function sortResultItems(items: Int32Array[]) {
  const geometries: Int32Array[] = [];
  const instances: Int32Array[] = [];
  const meshes: Int32Array[] = [];
  const lines: Int32Array[] = [];
  for (const item of items) {
    if (item[0] === RESULT_TAGS.geometry) geometries.push(item);
    else if (item[0] === RESULT_TAGS.instances) instances.push(item);
    else if (item[0] === RESULT_TAGS.mesh) meshes.push(item);
    else if (item[0] === RESULT_TAGS.lines) lines.push(item);
  }
  return { geometries, instances, meshes, lines };
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
  /** material group per vertex */
  groups?: Int32Array;
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

/**
 * Plantarium geometry: `[1, v, f, path id, ...faces, ...positions, ...normals, ...colors?]`, the
 * colors are there when the length leaves room for one per vertex
 */
export function readGeometry(data: Int32Array) {
  const geometry = readLayout(data, 4);
  const colorsStart = 4 + geometry.faceCount * 3 + geometry.vertexCount * 6;
  if (geometry.vertexCount > 0 && data.length === colorsStart + geometry.vertexCount) {
    geometry.colors = colorsAt(data, colorsStart, geometry.vertexCount);
  }
  return geometry;
}

/** Set in the flags word of a Myzel mesh when it brings one color per vertex */
export const MESH_HAS_COLORS = 1;

/** Myzel mesh: `[14, v, f, flags, ...faces, ...positions, ...normals, ...colors?, ...groups]` */
export function readMesh(data: Int32Array) {
  const mesh = readLayout(data, 4);
  const withColors = (data[3] & MESH_HAS_COLORS) !== 0;
  const colorsStart = 4 + mesh.faceCount * 3 + mesh.vertexCount * 6;
  const groupsStart = colorsStart + (withColors ? mesh.vertexCount : 0);
  const expected = groupsStart + mesh.vertexCount;
  if (data.length !== expected) {
    throw new Error(
      `A mesh with ${mesh.vertexCount} vertices, ${mesh.faceCount} faces and flags ${
        data[3]
      } needs ${expected} words, got ${data.length}`
    );
  }
  if (withColors) mesh.colors = colorsAt(data, colorsStart, mesh.vertexCount);
  mesh.groups = data.subarray(groupsStart, expected);
  return mesh;
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

export type GeometryData = MeshData & {
  /** the id of the path the geometry was made from, `NO_PATH` (-1) for none */
  pathId: number;
};

export type DecodedResult = {
  /** Plantarium geometry, one per path */
  geometries: GeometryData[];
  /** Plantarium instances like leaves, one shape and a matrix per instance */
  instances: InstancesData[];
  /** Myzel meshes with material groups and, for some, colors */
  meshes: MeshData[];
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
  const geometries = sorted.geometries.map(data => ({
    ...readGeometry(data),
    pathId: readGeometryPathId(data)
  }));
  const instances = sorted.instances.map(readInstances);
  const meshes = sorted.meshes.map(readMesh);

  let vertexCount = 0;
  let faceCount = 0;
  for (const item of [...geometries, ...meshes]) {
    vertexCount += item.vertexCount;
    faceCount += item.faceCount;
  }
  for (const item of instances) {
    vertexCount += item.vertexCount * item.instanceCount;
    faceCount += item.faceCount * item.instanceCount;
  }
  return { geometries, instances, meshes, vertexCount, faceCount };
}
