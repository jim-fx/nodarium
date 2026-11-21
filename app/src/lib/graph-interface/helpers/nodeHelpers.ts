import type { Edge, NodeDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { GROUP_INPUT } from '@nodarium/types';
import { toPlain } from './index.ts';

// values that only come in through a connection, so their parameter shows no input field
const SOCKET_ONLY_TYPES = new Set([
  'path',
  'geometry',
  'force',
  'points',
  'mesh',
  'pattern'
]);

export function getParameterHeight(node: NodeDefinition, inputKey: string) {
  if (node.id === GROUP_INPUT) {
    return 50;
  }

  const input = node.inputs?.[inputKey];
  if (!input) {
    return 0;
  }

  if (inputKey === 'seed' || input.type === 'creator') return 0;
  if (!node.inputs) return 0;
  if ('setting' in input) return 0;
  if (input.hidden) return 0;

  if ((input.type === 'shape' || input.type === 'curve') && input.external !== true) {
    return 200;
  }
  if (input.type === 'text' && input.multiline) {
    return 180;
  }
  if (input?.label !== '' && !input.external && !SOCKET_ONLY_TYPES.has(input.type)) {
    return 100;
  }
  return 50;
}

/** Where a node sits. The editor places every node when it loads a graph, see placeNodes */
export function positionOf(node: SerializedNode): [number, number] {
  return node.position ?? [0, 0];
}

export function serializeNode(node: SerializedNode): SerializedNode {
  const serialized: SerializedNode = {
    id: node.id,
    position: toPlain(node.position),
    type: node.type
  };
  if (node.props) serialized.props = toPlain(node.props);
  if (node.exposed?.length) serialized.exposed = toPlain(node.exposed);
  if (node.muted) serialized.muted = true;
  return serialized;
}

export function serializeEdge(edge: SerializedEdge | Edge): SerializedEdge {
  if (typeof edge[0] === 'number' && typeof edge[2] === 'number') {
    return [edge[0], edge[1], edge[2], edge[3]];
  }
  const e = edge as Edge;
  return [e[0].id, e[1], e[2].id, e[3]];
}

// The first output sits on the header, every other output gets a row below it
export const OUTPUT_ROW_HEIGHT = 5;
export function getOutputRowCount(node: NodeDefinition | undefined) {
  return Math.max(0, (node?.outputs?.length ?? 0) - 1);
}

// Keyed by definition, nodes with exposed inputs have their own definition
const nodeHeightCache = new WeakMap<NodeDefinition, number>();
export function getNodeHeight(node: NodeDefinition) {
  if (!node) return 5;
  if (!('inputs' in node)) {
    return 5 + getOutputRowCount(node) * OUTPUT_ROW_HEIGHT;
  }
  const cached = nodeHeightCache.get(node);
  if (cached !== undefined) {
    return cached;
  }
  let height = 5 + getOutputRowCount(node) * OUTPUT_ROW_HEIGHT;

  for (const key in node.inputs) {
    const h = getParameterHeight(node, key) / 10;
    height += h;
  }

  nodeHeightCache.set(node, height);
  return height;
}

// distance from the top of a node to the socket of one of its inputs
export function getInputSocketOffset(node: NodeDefinition | undefined, inputKey: string) {
  let height = 5 + getOutputRowCount(node) * OUTPUT_ROW_HEIGHT;
  for (const key in node?.inputs ?? {}) {
    const h = getParameterHeight(node!, key) / 10;
    if (key === inputKey) {
      return height + h / 2;
    }
    height += h;
  }
  return height;
}

// distance from the top of a node to the socket of one of its outputs
export function getOutputSocketOffset(nodeType: string, index: number) {
  if (nodeType === GROUP_INPUT) {
    return 2.5 + 5 * index + 5;
  }
  return 2.5 + OUTPUT_ROW_HEIGHT * index;
}

export type SocketSide = 'left' | 'right' | undefined;

/** Where the header shows the socket of the first output */
export function headerSocketSide(nodeType: string, node: NodeDefinition | undefined): SocketSide {
  return node?.outputs?.length && nodeType !== GROUP_INPUT ? 'right' : undefined;
}

/** Where the row of an input shows its socket. The rows of the Group Input node are its outputs. */
export function inputSocketSide(
  nodeType: string,
  input: { internal?: boolean } | undefined
): SocketSide {
  if (nodeType === GROUP_INPUT) return 'right';
  return input && input.internal !== true ? 'left' : undefined;
}

export type NodeRow = {
  /** in graph units */
  height: number;
  socket: SocketSide;
  /** the output index or the input key, as the socket ids of possibleSocketIds have it */
  socketIndex: number | string;
};

/** The rows below the header, top to bottom, as NodeHTML draws them */
export function getNodeRows(nodeType: string, node: NodeDefinition | undefined): NodeRow[] {
  const rows: NodeRow[] = [];
  for (let i = 1; i < (node?.outputs?.length ?? 0); i++) {
    rows.push({ height: OUTPUT_ROW_HEIGHT, socket: 'right', socketIndex: i });
  }
  const keys = Object.keys(node?.inputs ?? {});
  keys.forEach((key, i) => {
    const height = getParameterHeight(node!, key) / 10;
    if (!height) return;
    rows.push({
      height,
      socket: inputSocketSide(nodeType, node!.inputs![key]),
      socketIndex: nodeType === GROUP_INPUT ? i : key
    });
  });
  return rows;
}

// Whether an edge can be drawn into the input. Internal inputs are only set on the
// node itself, hidden inputs only take edges while they are exposed on the node.
export function isInputConnectable(input: { hidden?: boolean; internal?: boolean } | undefined) {
  return !!input && input.internal !== true && input.hidden !== true;
}

/**
 * The nodes with every hidden input an edge goes into added to their `exposed`, in the
 * order of the inputs of their type. The same array when no node misses one.
 */
export function exposeConnectedInputs<T extends SerializedNode>(
  nodes: T[],
  edges: SerializedEdge[],
  getType: (type: string) => NodeDefinition | undefined
): T[] {
  const connected = new Map<number, Set<string>>();
  for (const edge of edges) {
    let keys = connected.get(edge[2]);
    if (!keys) connected.set(edge[2], keys = new Set());
    keys.add(edge[3]);
  }

  let changed = false;
  const result = nodes.map((node) => {
    const keys = connected.get(node.id);
    const inputs = keys && getType(node.type)?.inputs;
    if (!inputs) return node;
    const missing = Object.keys(inputs).filter((key) =>
      keys.has(key)
      && inputs[key].hidden === true
      && inputs[key].internal !== true
      && !node.exposed?.includes(key)
    );
    if (!missing.length) return node;
    changed = true;
    return { ...node, exposed: [...(node.exposed ?? []), ...missing] };
  });
  return changed ? result : nodes;
}

export function areEdgesEqual(firstEdge: Edge, secondEdge: Edge) {
  if (firstEdge[0].id !== secondEdge[0].id) {
    return false;
  }

  if (firstEdge[1] !== secondEdge[1]) {
    return false;
  }

  if (firstEdge[2].id !== secondEdge[2].id) {
    return false;
  }

  if (firstEdge[3] !== secondEdge[3]) {
    return false;
  }

  return true;
}
