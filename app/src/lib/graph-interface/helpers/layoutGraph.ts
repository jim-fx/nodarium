import type { NodeDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { getInputSocketOffset, getNodeHeight, getOutputSocketOffset } from './nodeHelpers';

export const NODE_WIDTH = 20;
// space between two columns, and between two nodes in a column
export const COLUMN_GAP = 10;
const ROW_GAP = 5;
// the first input socket sits this much below the output socket, under the header of the node.
// Edges drop by it, so that a chain of nodes lines up at the top instead of stepping down
const SOCKET_DROP = 5;
// extra space between neighbours per step their branches need to meet, up to MAX_GROUP_DEPTH
const GROUP_GAP = 8;
const MAX_GROUP_DEPTH = 3;

// Arranges the nodes in columns, so that every edge runs from left to right. A node sits one
// column left of the leftmost node it feeds. Inside a column a node is placed next to the
// sockets it feeds in the next column. Overlapping nodes are merged into a stack that is
// centred on their targets, so the graph does not grow downwards, and unrelated branches get
// more space between them than siblings. The output node keeps its position. Returns the new position of every node.
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

  // nodes reachable from id in at most `steps` edges, including id itself
  function getDownstream(id: number, steps: number) {
    let frontier = [id];
    const seen = new Set(frontier);
    for (let step = 0; step < steps; step++) {
      frontier = frontier.flatMap((n) => outgoing.get(n)!.map((e) => e[2]))
        .filter((n) => !seen.has(n));
      for (const n of frontier) seen.add(n);
    }
    return seen;
  }

  // siblings that feed the same node sit ROW_GAP apart; the later their branches meet
  // downstream, the more GROUP_GAP is added, so separate sub groups stay visibly apart
  function getGap(a: number, b: number) {
    for (let depth = 1; depth < MAX_GROUP_DEPTH; depth++) {
      const reachA = getDownstream(a, depth);
      for (const n of getDownstream(b, depth)) {
        if (n !== a && n !== b && reachA.has(n)) return ROW_GAP + (depth - 1) * GROUP_GAP;
      }
    }
    return ROW_GAP + (MAX_GROUP_DEPTH - 1) * GROUP_GAP;
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

    const y = spreadColumn(
      placed.map((p) => p.y),
      placed.map((p) => getHeight(p.node)),
      placed.slice(1).map((p, i) => getGap(placed[i].node.id, p.node.id))
    );
    placed.forEach((p, i) => p.y = y[i]);

    // the column of the output node moves as a whole, so the output node keeps its position
    const placedAnchor = placed.find((p) => p.node === anchor);
    const shift = placedAnchor ? anchor.position[1] - placedAnchor.y : 0;
    for (const p of placed) {
      positions.set(p.node.id, [x, p.y + shift]);
    }
  }

  return positions;
}

// Moves the sorted boxes as little as possible from their wanted tops, so that box i+1 starts
// at least gaps[i] below the bottom of box i. Neighbours that would overlap are merged into one
// block, placed at the average of where its members want to be; merging repeats until no
// block overlaps the one above it.
function spreadColumn(wanted: number[], heights: number[], gaps: number[]): number[] {
  const blocks: { first: number; offsets: number[]; sum: number; height: number }[] = [];
  const top = (b: (typeof blocks)[number]) => b.sum / b.offsets.length;

  for (let i = 0; i < wanted.length; i++) {
    let block = { first: i, offsets: [0], sum: wanted[i], height: heights[i] };
    while (blocks.length > 0) {
      const above = blocks[blocks.length - 1];
      const gap = gaps[block.first - 1];
      if (top(above) + above.height + gap <= top(block)) break;
      blocks.pop();
      const shift = above.height + gap;
      block = {
        first: above.first,
        offsets: [...above.offsets, ...block.offsets.map((o) => o + shift)],
        sum: above.sum + block.sum - shift * block.offsets.length,
        height: shift + block.height
      };
    }
    blocks.push(block);
  }

  const result: number[] = [];
  for (const block of blocks) {
    for (const offset of block.offsets) result.push(top(block) + offset);
  }
  return result;
}
