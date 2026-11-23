import type { Edge, NodeInstance } from '@nodarium/runtime';

export type Relations = {
  parents: Map<number, NodeInstance[]>;
  children: Map<number, NodeInstance[]>;
};

function addOnce(map: Map<number, NodeInstance[]>, id: number, node: NodeInstance) {
  const list = map.get(id);
  if (!list) map.set(id, [node]);
  else if (!list.includes(node)) list.push(node);
}

/** The direct parents and children of every node, each once even with several edges between them */
export function buildRelations(edges: Edge[]): Relations {
  const parents = new Map<number, NodeInstance[]>();
  const children = new Map<number, NodeInstance[]>();
  // Only the node slots are read, a derived over this doesn't depend on the socket slots
  for (const edge of edges) {
    addOnce(children, edge[0].id, edge[2]);
    addOnce(parents, edge[2].id, edge[0]);
  }
  return { parents, children };
}

/** Every node reachable from `node` through `next`, each once */
export function reachable(node: NodeInstance, next: Map<number, NodeInstance[]>): NodeInstance[] {
  const visited = new Map<number, NodeInstance>();
  const stack = [...(next.get(node.id) ?? [])];
  while (stack.length) {
    const n = stack.pop()!;
    if (visited.has(n.id)) continue;
    visited.set(n.id, n);
    stack.push(...(next.get(n.id) ?? []));
  }
  return [...visited.values()];
}
