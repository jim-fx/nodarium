import { GEOMETRY_FLAGS, NO_CREATOR, readGeometry, RESULT_TAGS } from '@nodarium/runtime';
import { isPath, NO_PATH, type PathLink, readPathLink, readPathLinkTable } from '@nodarium/utils';

/** A part of the result under the pointer */
export type PartHit = {
  kind: 'geometry' | 'instances' | 'groups';
  /** the path the geometry was made from or the instance sits on, `NO_PATH` without one */
  pathId: number;
  /** the vertex group under the pointer, for a geometry with groups */
  group?: number;
  /**
   * the creator id of the graph node that made that group, when the geometry knows it, or that
   * placed the instance
   */
  creator?: number;
};

/**
 * The links of the paths of a result, by path id: those among its items, and those a geometry
 * carries of the paths it was made from, which `to_geometry` writes
 */
export function readPathLinks(items: Int32Array[]) {
  const links = new Map<number, PathLink>();
  for (const item of items) {
    if (isPath(item)) {
      const link = readPathLink(item);
      links.set(link.id, link);
    } else if (item[0] === RESULT_TAGS.geometry && item[4] & GEOMETRY_FLAGS.paths) {
      const table = readGeometry(item).paths;
      for (const link of table ? readPathLinkTable(table) : []) links.set(link.id, link);
    }
  }
  return links;
}

/** What the pool keeps in the `userData` of an object that a pick can hit */
type PickedObjectData = {
  /** of the geometry of a path */
  pathId?: number;
  /** of every instance, the path it sits on */
  pathIds?: ArrayLike<number>;
  /** of instances, the creator id of the node that placed them */
  creator?: number;
  /** of a geometry with groups that knows them, by group the creator id of its node */
  creators?: ArrayLike<number>;
};

/** The part an object of the result shows at `index`, its instance or its vertex group */
export function partHit(
  kind: PartHit['kind'],
  data: PickedObjectData,
  index: number
): PartHit {
  if (kind === 'geometry') return { kind, pathId: data.pathId ?? NO_PATH };
  if (kind === 'instances') {
    const pathId = data.pathIds?.[index] ?? NO_PATH;
    const unknown = data.creator === undefined || data.creator === NO_CREATOR;
    return unknown ? { kind, pathId } : { kind, pathId, creator: data.creator };
  }
  return { kind, pathId: NO_PATH, group: index, creator: data.creators?.[index] };
}
