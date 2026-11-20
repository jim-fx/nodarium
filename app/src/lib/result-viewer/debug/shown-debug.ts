import { type Graph, isDebugNode } from '@nodarium/types';

/** A debug node and the node whose output it shows */
export type DebugLink = { debugNode: number; source: number };

/** The debug nodes of a graph as it was run and the nodes they are connected to */
export function debugLinks(graph: Pick<Graph, 'nodes' | 'edges'>): DebugLink[] {
  const debugNodes = new Set(graph.nodes.filter((n) => isDebugNode(n.type)).map((n) => n.id));
  // amount and clip can be connected as well, they only change how the output is drawn
  return graph.edges
    .filter((edge) => edge[3] === 'input' && debugNodes.has(edge[2]))
    .map((edge) => ({ debugNode: edge[2], source: edge[0] }));
}

/**
 * The nodes the debug data of a run is of, in the order they were first debugged: the order of
 * the last run without the ones that are gone, and the new ones at the end
 */
export function debugOrder(previous: number[], sources: number[]) {
  const present = new Set(sources);
  const kept = previous.filter((id) => present.has(id));
  const known = new Set(kept);
  return [...kept, ...sources.filter((id) => !known.has(id))];
}

/**
 * The node whose debug data the viewer shows when `activeNodeId` is selected: the one a selected
 * debug node is connected to, or the selected node when a debug node is connected to it.
 * Undefined when the selection has nothing to do with a debug node.
 */
export function selectedDebugSource(
  links: DebugLink[],
  sources: number[],
  activeNodeId: number | undefined
) {
  if (activeNodeId === undefined) return;
  const present = new Set(sources);
  const watched = links.find((link) => link.debugNode === activeNodeId)?.source;
  if (watched !== undefined && present.has(watched)) return watched;
  if (present.has(activeNodeId)) return activeNodeId;
}

/**
 * The one node whose debug data the viewer shows: the one of the selection, otherwise the one
 * that was debugged last
 */
export function shownDebugSource(
  links: DebugLink[],
  order: number[],
  activeNodeId: number | undefined
) {
  return selectedDebugSource(links, order, activeNodeId) ?? order.at(-1);
}
