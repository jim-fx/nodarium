import type { NodeDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { getInputSocketOffset, getNodeHeight, getOutputSocketOffset } from './nodeHelpers';

const NODE_WIDTH = 20;
// space between two columns, and between two nodes in a column
const COLUMN_GAP = 10;
const ROW_GAP = 5;
// the first input socket sits this much below the output socket, under the header of the node.
// Edges drop by it, so that a chain of nodes lines up at the top instead of stepping down
const SOCKET_DROP = 5;

// Arranges the nodes in columns, so that every edge runs from left to right. A node sits one
// column left of the leftmost node it feeds. Inside a column a node is placed next to the
// sockets it feeds in the next column, and pushed down where it would overlap the node above.
// The output node keeps its position. Returns the new position of every node.
export function layoutGraph<T extends SerializedNode>(
  nodes: T[],
  edges: SerializedEdge[],
  getDefinition: (node: T) => NodeDefinition | undefined
): Map<number, [number, number]> {
  const positions = new Map<number, [number, number]>();
  if (nodes.length === 0) return positions;

  const byId = new Map(nodes.map((n) => [n.id, n]));
  const outgoing = new Map<number, SerializedEdge[]>(nodes.map((n) => [n.id, []]));
  for (const edge of edges) {
    if (byId.has(edge[0]) && byId.has(edge[2]) && edge[0] !== edge[2]) {
      outgoing.get(edge[0])!.push(edge);
    }
  }

  // columns count from the right, a node that feeds nothing is in column 0
  const columns = new Map<number, number>();
  const visiting = new Set<number>();
  function getColumn(id: number): number {
    const known = columns.get(id);
    if (known !== undefined) return known;
    // only a cycle brings us back to a node we are visiting
    if (visiting.has(id)) return 0;
    visiting.add(id);
    let column = 0;
    for (const edge of outgoing.get(id)!) {
      column = Math.max(column, getColumn(edge[2]) + 1);
    }
    visiting.delete(id);
    columns.set(id, column);
    return column;
  }

  const byColumn: T[][] = [];
  for (const node of nodes) {
    (byColumn[getColumn(node.id)] ??= []).push(node);
  }

  function getHeight(node: T) {
    const definition = getDefinition(node);
    return definition ? getNodeHeight(definition) : 20;
  }

  // next to the sockets it feeds in the next column, its edges drop into them by SOCKET_DROP.
  // Nodes that feed nothing stay where they are
  function getTargetY(node: T, column: number) {
    const next = outgoing.get(node.id)!.filter((e) => columns.get(e[2]) === column - 1);
    if (next.length === 0) return node.position[1];
    let sum = 0;
    for (const [, outputIndex, targetId, inputKey] of next) {
      sum += positions.get(targetId)![1]
        + getInputSocketOffset(getDefinition(byId.get(targetId)!), inputKey)
        - getOutputSocketOffset(node.type, outputIndex)
        - SOCKET_DROP;
    }
    return sum / next.length;
  }

  // column 0 is only empty when the graph has a cycle
  const anchor = nodes.find((n) => n.type.endsWith('/output')) ?? byColumn.find(Boolean)![0];

  for (let column = 0; column < byColumn.length; column++) {
    const x = anchor.position[0] - column * (NODE_WIDTH + COLUMN_GAP);
    const placed = (byColumn[column] ?? [])
      .map((node) => ({ node, y: getTargetY(node, column) }))
      .sort((a, b) =>
        a.y - b.y || a.node.position[1] - b.node.position[1] || a.node.id - b.node.id
      );

    let bottom = -Infinity;
    for (const p of placed) {
      p.y = Math.max(p.y, bottom + ROW_GAP);
      bottom = p.y + getHeight(p.node);
    }

    // the column of the output node moves as a whole, so the output node keeps its position
    const placedAnchor = placed.find((p) => p.node === anchor);
    const shift = placedAnchor ? anchor.position[1] - placedAnchor.y : 0;
    for (const p of placed) {
      positions.set(p.node.id, [x, p.y + shift]);
    }
  }

  return positions;
}
