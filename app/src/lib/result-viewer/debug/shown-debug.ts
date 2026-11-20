import { expandWithChains } from '@nodarium/runtime';
import { type Graph, isDebugNode } from '@nodarium/types';

/**
 * A debug node and the node whose output it shows, by their ids in the graph as it runs, with
 * groups expanded. The chains lead from the outermost group instance to the node in the editor.
 */
export type DebugLink = {
  debugNode: number;
  source: number;
  debugChain: number[];
  sourceChain: number[];
};

/** The debug nodes of a graph as it was run, also those inside groups, and the nodes they show */
export function debugLinks(graph: Graph): DebugLink[] {
  const { graph: expanded, chains } = expandWithChains(graph);
  const debugNodes = new Set(expanded.nodes.filter((n) => isDebugNode(n.type)).map((n) => n.id));
  // amount and clip can be connected as well, they only change how the output is drawn
  return expanded.edges
    .filter((edge) => edge[3] === 'input' && debugNodes.has(edge[2]))
    .map((edge) => ({
      debugNode: edge[2],
      source: edge[0],
      debugChain: chains.get(edge[2]) ?? [edge[2]],
      sourceChain: chains.get(edge[0]) ?? [edge[0]]
    }));
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

/** The node of `chain` that the editor shows inside the groups of `entered` */
function visibleAt(chain: number[], entered: number[]) {
  if (chain.length <= entered.length) return;
  return entered.every((id, i) => chain[i] === id) ? chain[entered.length] : undefined;
}

/**
 * The node whose debug data the viewer shows when `activeNodeId` is selected inside the groups of
 * `entered`: the one a selected debug node is connected to, or the one behind the selected node
 * when a debug node next to it is connected to it. Undefined when the selection has nothing to
 * do with a debug node.
 */
export function selectedDebugSource(
  links: DebugLink[],
  sources: number[],
  activeNodeId: number | undefined,
  entered: number[] = []
) {
  if (activeNodeId === undefined) return;
  const present = new Set(sources);
  const onLevel = links.filter((link) =>
    present.has(link.source) && link.debugChain.length === entered.length + 1
    && visibleAt(link.debugChain, entered) !== undefined
  );
  return (
    onLevel.find((link) => visibleAt(link.debugChain, entered) === activeNodeId)
      ?? onLevel.find((link) => visibleAt(link.sourceChain, entered) === activeNodeId)
  )?.source;
}

/**
 * The one node whose debug data the viewer shows: the one of the selection, otherwise the one
 * that was debugged last
 */
export function shownDebugSource(
  links: DebugLink[],
  order: number[],
  activeNodeId: number | undefined,
  entered: number[] = []
) {
  return selectedDebugSource(links, order, activeNodeId, entered) ?? order.at(-1);
}
