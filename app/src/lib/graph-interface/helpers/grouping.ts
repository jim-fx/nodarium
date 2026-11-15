import {
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  groupInputKey,
  groupInputValue,
  groupOutputIndex
} from '@nodarium/types';
import type { GroupDefinition, NodeInput, SerializedEdge, SerializedNode } from '@nodarium/types';
import { toPlain } from './index.ts';
import { serializeNode } from './nodeHelpers';

type Level = { nodes: SerializedNode[]; edges: SerializedEdge[] };

const isBoundary = (node: SerializedNode) =>
  node.type === GROUP_INPUT || node.type === GROUP_OUTPUT;

const mean = (values: number[]) => values.reduce((sum, v) => sum + v, 0) / values.length;

const sourceSocket = (edge: SerializedEdge) => `${edge[0]}-${edge[1]}`;

/**
 * One edge per socket the edges leave, in the order the sockets first appear. Several edges
 * leaving one socket become one input or output of the group, the last of them stands for it.
 */
function bySourceSocket(edges: SerializedEdge[]) {
  return new Map(edges.map((edge) => [sourceSocket(edge), edge]));
}

function uniqueEdges(edges: SerializedEdge[]) {
  const seen = new Set<string>();
  return edges.filter((edge) => {
    const key = edge.join('-');
    return !seen.has(key) && !!seen.add(key);
  });
}

/**
 * Moves the nodes `ids` of a level into a new group, in the order of `ids`. The Group Input and
 * Group Output node of the level can't be grouped.
 *
 * Every socket outside that feeds the selection becomes an input of the group, every socket of
 * the selection that feeds a node outside an output.
 */
export function groupSelection(
  level: Level,
  ids: Set<number>,
  context: {
    outputType: (nodeId: number, output: number) => string | undefined;
    input: (nodeId: number, key: string) => NodeInput | undefined;
    nextId: () => number;
  }
): { group: GroupDefinition; instance: SerializedNode; edges: SerializedEdge[] } | undefined {
  const nodesById = new Map(level.nodes.map((n) => [n.id, n]));
  const nodes = [...ids].flatMap((id) => nodesById.get(id) ?? []).filter((n) => !isBoundary(n));
  if (!nodes.length) return;

  const inside = new Set(nodes.map((n) => n.id));
  const incoming = bySourceSocket(level.edges.filter((e) => inside.has(e[2]) && !inside.has(e[0])));
  const outgoing = bySourceSocket(level.edges.filter((e) => inside.has(e[0]) && !inside.has(e[2])));
  const inputIndex = new Map([...incoming.keys()].map((socket, i) => [socket, i]));
  const outputIndex = new Map([...outgoing.keys()].map((socket, i) => [socket, i]));

  const inputs = Object.fromEntries(
    [...incoming.values()].map(([fromId, fromSocket, toId, key], i) => {
      const internal = context.input(toId, key);
      const type = context.outputType(fromId, fromSocket) || '*';
      // The widget of the group node starts at the value the internal node runs with
      const value = internal?.type === type
        ? nodesById.get(toId)?.props?.[key] ?? internal.value
        : undefined;
      const input = {
        label: internal?.label ?? key,
        type,
        ...(value !== undefined && { value: toPlain(value) })
      };
      return [`input_${i}`, input as NodeInput];
    })
  );

  const outputs = [...outgoing.values()].map(([fromId, fromSocket]) => {
    const type = context.outputType(fromId, fromSocket) || '*';
    return {
      label: type === '*' ? 'Output' : type.charAt(0).toUpperCase() + type.slice(1),
      type
    };
  });

  const xs = nodes.map((n) => n.position[0]);
  const ys = nodes.map((n) => n.position[1]);
  const middleY = (Math.min(...ys) + Math.max(...ys)) / 2;

  const groupInput: SerializedNode = {
    id: context.nextId(),
    type: GROUP_INPUT,
    position: [Math.min(...xs) - 50, middleY]
  };
  const groupOutput: SerializedNode = {
    id: context.nextId(),
    type: GROUP_OUTPUT,
    position: [Math.max(...xs) + 25, middleY]
  };
  const groupId = context.nextId();
  const instance: SerializedNode = {
    id: context.nextId(),
    type: GROUP_INSTANCE,
    position: [mean(xs), mean(ys)],
    props: { groupId }
  };

  // Every edge that touches the selection is in the group: as it is between two of its nodes,
  // from the Group Input or into the Group Output node where it crosses the boundary. Outside
  // the crossing edges go to the group node, the ones between two of its nodes are gone.
  const innerEdges: SerializedEdge[] = [];
  const outerEdges: SerializedEdge[] = [];
  for (const edge of level.edges) {
    const [fromId, fromSocket, toId, key] = edge;
    const socket = sourceSocket(edge);
    if (inside.has(fromId) && inside.has(toId)) {
      innerEdges.push([fromId, fromSocket, toId, key]);
    } else if (inside.has(toId)) {
      const index = inputIndex.get(socket)!;
      innerEdges.push([groupInput.id, index, toId, key]);
      outerEdges.push([fromId, fromSocket, instance.id, `input_${index}`]);
    } else if (inside.has(fromId)) {
      const index = outputIndex.get(socket)!;
      innerEdges.push([fromId, fromSocket, groupOutput.id, `out_${index}`]);
      outerEdges.push([instance.id, index, toId, key]);
    } else {
      outerEdges.push(edge);
    }
  }

  return {
    group: {
      id: groupId,
      inputs,
      outputs,
      nodes: [groupInput, ...nodes, groupOutput].map((n) => serializeNode(n)),
      edges: uniqueEdges(innerEdges)
    },
    instance,
    edges: uniqueEdges(outerEdges)
  };
}

/**
 * Brings the nodes of a group back out in place of one of its instances. Every node of the
 * group gets a new id, the group and its other instances stay as they are.
 *
 * An edge of the group that crosses its boundary becomes an edge to what the instance was
 * connected to. An input of the group nothing was connected to leaves the value the instance ran
 * with on the nodes it fed.
 */
export function ungroupInstance(
  level: Level,
  instance: SerializedNode,
  group: GroupDefinition,
  nextId: () => number
): { nodes: SerializedNode[]; edges: SerializedEdge[] } {
  const newIds = new Map(group.nodes.map((n) => [n.id, nextId()]));
  const inner = group.nodes.filter((n) => !isBoundary(n));
  const groupInputId = group.nodes.find((n) => n.type === GROUP_INPUT)?.id;
  const groupOutputId = group.nodes.find((n) => n.type === GROUP_OUTPUT)?.id;

  // The nodes keep their layout, their centre is where the instance was
  const offsetX = inner.length ? instance.position[0] - mean(inner.map((n) => n.position[0])) : 0;
  const offsetY = inner.length ? instance.position[1] - mean(inner.map((n) => n.position[1])) : 0;

  const nodes = new Map(inner.map((n): [number, SerializedNode] => [n.id, {
    id: newIds.get(n.id)!,
    type: n.type,
    position: [n.position[0] + offsetX, n.position[1] + offsetY],
    props: toPlain(n.props) ?? {},
    ...(n.exposed?.length ? { exposed: [...n.exposed] } : {})
  }]));

  const incoming = level.edges.filter((e) => e[2] === instance.id);
  const outgoing = level.edges.filter((e) => e[0] === instance.id);

  const edges: SerializedEdge[] = [];
  for (const [fromId, fromSocket, toId, toSocket] of group.edges) {
    let source: [number, number];
    if (fromId === groupInputId) {
      const key = groupInputKey(group, fromSocket);
      if (key === undefined) continue;
      const external = incoming.findLast((e) => e[3] === key);
      if (!external) {
        const value = groupInputValue(instance, group, key);
        const props = nodes.get(toId)?.props;
        if (value !== undefined && props) props[toSocket] = toPlain(value) as typeof props[string];
        continue;
      }
      source = [external[0], external[1]];
    } else if (nodes.has(fromId)) {
      source = [newIds.get(fromId)!, fromSocket];
    } else {
      continue;
    }

    if (toId === groupOutputId) {
      const output = groupOutputIndex(toSocket);
      for (const external of outgoing) {
        if (external[1] === output) edges.push([...source, external[2], external[3]]);
      }
    } else if (nodes.has(toId)) {
      edges.push([...source, newIds.get(toId)!, toSocket]);
    }
  }

  return { nodes: [...nodes.values()], edges };
}
