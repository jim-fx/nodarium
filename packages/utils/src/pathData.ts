/**
 * Readers for the structure that paths, their geometry and their instances carry, see
 * `docs/PLANTARIUM.md`. All of them read an item as `splitNestedArray` returns it: without the
 * surrounding brackets, starting at the type.
 */

/** type, depth, id, parent id, parent alpha, creator and index */
export const PATH_HEADER_SIZE = 7;

/** The parent id of a path that grows on its own. */
export const NO_PARENT = -1;

/** The path id of geometry and instances that don't come from a path. */
export const NO_PATH = -1;

export type PathLink = {
  /** Unique in a list of paths, a hash of creator, parent and index. */
  id: number;
  /** The id of the path this one grows on, `NO_PARENT` if it has none. */
  parentId: number;
  /** Where on the parent the path grows, from 0 at its base to 1 at its tip. */
  parentAlpha: number;
  /** The id of the graph node that made the path. */
  creator: number;
  /** The number of the path among the paths its creator made on the same parent. */
  index: number;
};

export function isPath(item: Int32Array) {
  return item[0] === 0 && item.length >= PATH_HEADER_SIZE;
}

export function readPathLink(path: Int32Array): PathLink {
  return {
    id: path[2],
    parentId: path[3],
    parentAlpha: new Float32Array(path.buffer, path.byteOffset + 4 * 4, 1)[0],
    creator: path[5],
    index: path[6]
  };
}

/** The id of the path an extruded geometry was made from, `NO_PATH` for other geometry. */
export function readGeometryPathId(geometry: Int32Array) {
  return geometry[3];
}

/** For every instance the id of the path it sits on, `NO_PATH` for none. */
export function readInstancePathIds(instances: Int32Array) {
  const [, vertexCount, faceCount, instanceCount] = instances;
  // type, vertex, face and instance amount and the stem depth, then faces, positions, normals
  // and a 4x4 matrix per instance
  const start = 5 + faceCount * 3 + vertexCount * 6 + instanceCount * 16;
  return instances.subarray(start, start + instanceCount);
}

/** The ids of the paths between `id` and the one that grows on its own, `id` itself first. */
export function pathLineage(links: Iterable<PathLink>, id: number): number[] {
  const byId = new Map<number, PathLink>();
  for (const link of links) byId.set(link.id, link);

  const lineage: number[] = [];
  for (
    let link = byId.get(id);
    link && !lineage.includes(link.id);
    link = byId.get(link.parentId)
  ) {
    lineage.push(link.id);
  }
  return lineage;
}
