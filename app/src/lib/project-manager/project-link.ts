import { graphLink } from '@nodarium/runtime';
import type { Graph } from '@nodarium/types';

/** The `#graph=` link of a project, File → Copy link copies it */
export function projectLink(graph: Graph, origin: string) {
  // the id and the time of the last change are those of the project, not of the graph
  const { version, meta, settings, nodes, edges, groups } = graph;
  return graphLink(
    { version, meta: { title: meta?.title, unit: meta?.unit }, settings, nodes, edges, groups },
    origin
  );
}
