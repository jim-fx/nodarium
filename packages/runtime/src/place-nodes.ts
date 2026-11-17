import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';

/** The width of a node in the editor and the gap between two columns, as "Format nodes" has them */
const COLUMN_WIDTH = 30;
const ROW_GAP = 5;
/** How tall a node is taken to be when `height` doesn't say */
const DEFAULT_HEIGHT = 30;

export type PlacedNode = SerializedNode & { position: [number, number] };

export type PlaceOptions = {
  /** The height of a node in the units of its position, so the nodes of a column don't overlap */
  height?: (node: SerializedNode) => number;
};

/** `position` moved by x and y, undefined for a node that has no position yet */
export function offsetPosition(
  position: [number, number] | undefined,
  x: number,
  y: number
): [number, number] | undefined {
  return position && [position[0] + x, position[1] + y];
}

function needsPlace(nodes: SerializedNode[]) {
  return nodes.some((node) => !Array.isArray(node.position));
}

/**
 * The nodes of one level with a position for each: the nodes without one go into columns by
 * their depth, a node one column left of the leftmost node it feeds, like "Format nodes" of the
 * editor sorts them, stacked from the top of the output node down. Nodes that have a position
 * keep it.
 */
function placeLevel(
  nodes: SerializedNode[],
  edges: SerializedEdge[],
  height: (node: SerializedNode) => number
): PlacedNode[] {
  const ids = new Set(nodes.map((node) => node.id));
  const targets = new Map<number, number[]>();
  for (const [from, , to] of edges) {
    if (from === to || !ids.has(from) || !ids.has(to)) continue;
    targets.set(from, [...(targets.get(from) ?? []), to]);
  }

  const columns = new Map<number, number>();
  const visiting = new Set<number>();
  const columnOf = (id: number): number => {
    const known = columns.get(id);
    if (known !== undefined) return known;
    // only a cycle leads back to a node that is being visited
    if (visiting.has(id)) return 0;
    visiting.add(id);
    let column = 0;
    for (const target of targets.get(id) ?? []) column = Math.max(column, columnOf(target) + 1);
    visiting.delete(id);
    columns.set(id, column);
    return column;
  };

  const anchor = nodes.find((node) => node.type.endsWith('/output') && node.position)?.position
    ?? [0, 0];
  const nextY = new Map<number, number>();
  return nodes.map((node) => {
    if (Array.isArray(node.position)) return node as PlacedNode;
    const column = columnOf(node.id);
    const y = nextY.get(column) ?? anchor[1];
    nextY.set(column, y + height(node) + ROW_GAP);
    return { ...node, position: [anchor[0] - column * COLUMN_WIDTH, y] };
  });
}

/**
 * The graph with a position for every node, in the graph and in its groups. Nodes without one,
 * as in a graph written by hand, are placed in columns by their depth, see {@link placeLevel}.
 * Nodes with a position keep it. A graph whose nodes all have one is returned as it is.
 */
export function placeNodes(graph: Graph, options: PlaceOptions = {}): Graph {
  const groups = graph.groups ?? [];
  if (!needsPlace(graph.nodes) && !groups.some((group) => needsPlace(group.nodes))) return graph;
  const height = options.height ?? (() => DEFAULT_HEIGHT);
  return {
    ...graph,
    nodes: needsPlace(graph.nodes) ? placeLevel(graph.nodes, graph.edges, height) : graph.nodes,
    groups: groups.map((group) =>
      needsPlace(group.nodes)
        ? { ...group, nodes: placeLevel(group.nodes, group.edges, height) }
        : group
    )
  };
}
