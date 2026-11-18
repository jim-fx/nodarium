import { isPath, type PathLink, readPathLink } from '@nodarium/utils';

/** A part of the result under the pointer */
export type PartHit = {
  kind: 'geometry' | 'instances' | 'mesh';
  /** the path the geometry was made from or the instance sits on, `NO_PATH` without one */
  pathId: number;
  /** the vertex group under the pointer, for a Myzel mesh */
  group?: number;
  /** the creator id of the graph node that made that group, when the mesh knows it */
  creator?: number;
};

/** The links of the paths among the items of a result, by path id */
export function readPathLinks(items: Int32Array[]) {
  const links = new Map<number, PathLink>();
  for (const item of items) {
    if (!isPath(item)) continue;
    const link = readPathLink(item);
    links.set(link.id, link);
  }
  return links;
}
