import type { Graph, SerializedNode } from '@nodarium/types';
import { fastHashArrayBuffer } from '@nodarium/utils';

// Only the parts of a node that influence the generated geometry,
// moving nodes around should not invalidate a thumbnail
function stripNodes(nodes: SerializedNode[]) {
  return nodes.map(({ id, type, props }) => ({ id, type, props }));
}

export function hashGraph(graph: Graph): string {
  return fastHashArrayBuffer(JSON.stringify({
    nodes: stripNodes(graph.nodes),
    edges: graph.edges,
    groups: graph.groups?.map((group) => ({ ...group, nodes: stripNodes(group.nodes) })),
    settings: graph.settings ?? {}
  }));
}
