import { CORE_COLLECTION, INTERNAL_PREFIX, type NodeDefinition } from '@nodarium/types';

export type AddMenuSection = {
  /** the collection, like `max/plantarium` */
  id: string;
  title: string;
  nodes: NodeDefinition[];
};

export function nodeLabel(node: Pick<NodeDefinition, 'id' | 'meta'>) {
  return node.meta?.title ?? node.id.split('/').at(-1) ?? node.id;
}

// the debug and group nodes are as basic as the value nodes
function collectionOf(node: NodeDefinition) {
  if (node.id.startsWith(INTERNAL_PREFIX)) return CORE_COLLECTION;
  const [user, collection] = node.id.split('/');
  return `${user}/${collection}`;
}

/** The nodes whose label or id contains `query`, ignoring case; those whose label starts with it first */
export function searchNodes(nodes: NodeDefinition[], query = '') {
  const q = query.trim().toLowerCase();
  if (!q) return nodes;
  const prefix: NodeDefinition[] = [];
  const rest: NodeDefinition[] = [];
  for (const node of nodes) {
    const label = nodeLabel(node).toLowerCase();
    if (label.startsWith(q)) prefix.push(node);
    else if (label.includes(q) || node.id.toLowerCase().includes(q)) rest.push(node);
  }
  return [...prefix, ...rest];
}

/**
 * One section per collection, core first and the others by name. The nodes keep their order,
 * a search puts the best matches first.
 */
export function groupNodes(nodes: NodeDefinition[]): AddMenuSection[] {
  const byCollection = new Map<string, NodeDefinition[]>();
  for (const node of nodes) {
    const id = collectionOf(node);
    byCollection.set(id, [...(byCollection.get(id) ?? []), node]);
  }

  const names = new Map<string, number>();
  for (const id of byCollection.keys()) {
    const name = id.split('/').at(-1)!;
    names.set(name, (names.get(name) ?? 0) + 1);
  }

  return [...byCollection.entries()]
    .map(([id, nodes]) => {
      const name = id.split('/').at(-1)!;
      return { id, title: names.get(name)! > 1 ? id : name, nodes };
    })
    .sort((a, b) => {
      if (a.id === CORE_COLLECTION) return -1;
      if (b.id === CORE_COLLECTION) return 1;
      return a.title.localeCompare(b.title);
    });
}

/** Sorted by label within each section, the order of the menu without a search */
export function sortByLabel(nodes: NodeDefinition[]) {
  return [...nodes].sort((a, b) => nodeLabel(a).localeCompare(nodeLabel(b)));
}
