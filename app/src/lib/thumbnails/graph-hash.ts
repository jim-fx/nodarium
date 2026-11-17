import type { Graph, SerializedNode } from '@nodarium/types';
import { fastHashArrayBuffer } from '@nodarium/utils';

// Only the parts of a node that influence the generated geometry,
// moving nodes around should not invalidate a thumbnail
function stripNodes(nodes: SerializedNode[]) {
  return nodes.map(({ id, type, props }) => ({ id, type, props }));
}

function sortKeys(_key: string, value: unknown) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value;
  return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)));
}

/**
 * With `ignoreKeyOrder`, a graph the editor saved, which writes its settings and groups in an
 * order of its own, has the hash of the file it was opened from. The thumbnails keep the hash
 * without it, the one their stored hashes were made with.
 */
export function hashGraph(graph: Graph, { ignoreKeyOrder = false } = {}): string {
  return fastHashArrayBuffer(JSON.stringify(
    {
      nodes: stripNodes(graph.nodes),
      edges: graph.edges,
      groups: graph.groups?.map((group) => ({ ...group, nodes: stripNodes(group.nodes) })),
      settings: graph.settings ?? {}
    },
    ignoreKeyOrder ? sortKeys : undefined
  ));
}
