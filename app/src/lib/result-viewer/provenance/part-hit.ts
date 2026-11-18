import { isPath, NO_PATH, type PathLink, readPathLink } from '@nodarium/utils';

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

/** What the pool keeps in the `userData` of an object that a pick can hit */
type PickedObjectData = {
  /** of the geometry of a path */
  pathId?: number;
  /** of every instance, the path it sits on */
  pathIds?: ArrayLike<number>;
  /** of a Myzel mesh that knows them, by group the creator id of its node */
  creators?: ArrayLike<number>;
};

/** The part an object of the result shows at `index`, its instance or its vertex group */
export function partHit(
  kind: PartHit['kind'],
  data: PickedObjectData,
  index: number
): PartHit {
  if (kind === 'geometry') return { kind, pathId: data.pathId ?? NO_PATH };
  if (kind === 'instances') return { kind, pathId: data.pathIds?.[index] ?? NO_PATH };
  return { kind, pathId: NO_PATH, group: index, creator: data.creators?.[index] };
}
