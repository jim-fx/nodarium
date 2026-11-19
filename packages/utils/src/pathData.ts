/**
 * Readers for the structure that paths, their geometry and their instances carry, see
 * `docs/PLANTARIUM.md`. All of them read an item as `splitNestedArray` returns it: without the
 * surrounding brackets, starting at the type.
 */

/** type, depth, id, parent id, parent alpha, creator, index and grown */
export const PATH_HEADER_SIZE = 8;

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
  /** How much of the path has grown: 1 when it is full-grown, below 1 that share of its final length. */
  grown: number;
};

/**
 * What a path carries at its points besides the points, its type is the sum of them. Each is one
 * word per point after the points, in this order: an RGBA8 color, and the fibres and the fill of
 * `hyphae_to_mesh` (f32).
 */
export const POINT_VALUES = { colors: 16, fibres: 32, fill: 64 } as const;

const ALL_POINT_VALUES = POINT_VALUES.colors | POINT_VALUES.fibres | POINT_VALUES.fill;

/** The type of a path with an RGBA8 color at every point and nothing else */
export const COLORED_PATH = POINT_VALUES.colors;

export function isPath(item: Int32Array) {
  return (item[0] & ~ALL_POINT_VALUES) === 0 && item.length >= PATH_HEADER_SIZE;
}

/** How many words a path has per point: 4 for the point and one for every value it carries */
function wordsPerPoint(path: Int32Array) {
  let words = 4;
  for (const flag of Object.values(POINT_VALUES)) if (path[0] & flag) words++;
  return words;
}

/** How many points a path has */
export function pathPointCount(path: Int32Array) {
  return Math.floor((path.length - PATH_HEADER_SIZE) / wordsPerPoint(path));
}

/** The RGBA8 colors of a path, sRGB with red first, four bytes per point, or undefined without */
export function readPathColors(path: Int32Array) {
  if (!(path[0] & POINT_VALUES.colors)) return undefined;
  const points = pathPointCount(path);
  const start = PATH_HEADER_SIZE + points * 4;
  return new Uint8Array(path.buffer, path.byteOffset + start * 4, points * 4);
}

export function readPathLink(path: Int32Array): PathLink {
  return linkAt(path, 2);
}

/** The words of a link in the table of paths of a geometry, see `GEOMETRY_FLAGS.paths` */
export const PATH_LINK_WORDS = 6;

/** The links of a table of paths: id, parent id, parent alpha, creator, index and grown each */
export function readPathLinkTable(table: Int32Array): PathLink[] {
  const links: PathLink[] = [];
  for (let at = 0; at + PATH_LINK_WORDS <= table.length; at += PATH_LINK_WORDS) {
    links.push(linkAt(table, at));
  }
  return links;
}

function linkAt(words: Int32Array, at: number): PathLink {
  const float = (i: number) => new Float32Array(words.buffer, words.byteOffset + i * 4, 1)[0];
  return {
    id: words[at],
    parentId: words[at + 1],
    parentAlpha: float(at + 2),
    creator: words[at + 3],
    index: words[at + 4],
    grown: float(at + 5)
  };
}

/** The id of the path a geometry `[1, v, f, path id, flags, ...]` was made from, `NO_PATH` for none. */
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
