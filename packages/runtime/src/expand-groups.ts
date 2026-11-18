import type { Graph, GroupDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import {
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  groupIdOf,
  groupInputKey,
  groupInputValue,
  groupOutputIndex
} from '@nodarium/types';
import { hashString53 } from '@nodarium/utils';

export type NodeSource = { instance: number; node: number };

/**
 * @param origins filled with the ID of the outermost group node every expanded
 * node comes from, so errors can point at a node that is visible in the editor
 * @param sources filled with the group node every expanded node was inlined for and the ID it
 * has inside its group
 */
export function expandGroups(
  graph: Graph,
  origins?: Map<number, number>,
  sources?: Map<number, NodeSource>
): Graph {
  if (!graph.groups || graph.groups.length === 0) return graph;

  const groups = new Map<number, GroupDefinition>();
  for (const group of graph.groups) if (!groups.has(group.id)) groups.set(group.id, group);
  assertNoGroupCycles(groups);

  // Expanded nodes get an ID derived from the instance they come from, so the
  // IDs (and the cache entries keyed by them) stay the same between runs. Unlike
  // an offset per level this stays in the safe integer range at any nesting depth.
  const usedIds = new Set(graph.nodes.map(n => n.id));
  function createExpandedId(instanceId: number, nodeId: number) {
    let id = hashString53(`${instanceId}/${nodeId}`);
    while (usedIds.has(id)) id = (id + 1) % Number.MAX_SAFE_INTEGER;
    usedIds.add(id);
    return id;
  }

  // an inlined group appends its nodes, the instances among them are expanded after the others
  const pending = [...graph.nodes];
  const nodes: SerializedNode[] = [];
  let edges = [...graph.edges];

  for (let i = 0; i < pending.length; i++) {
    const node = pending[i];
    const group = node.type === GROUP_INSTANCE
      ? groups.get(node.props?.groupId as number)
      : undefined;
    if (!group) {
      nodes.push(node);
      continue;
    }

    let replacement: SerializedEdge[];
    if (node.muted === true) {
      replacement = bypassInstance(node, group, edges);
    } else {
      const inlined = inlineInstance(node, group, edges, id => {
        const expandedId = createExpandedId(node.id, id);
        sources?.set(expandedId, { instance: node.id, node: id });
        return expandedId;
      });
      for (const n of inlined.nodes) origins?.set(n.id, origins.get(node.id) ?? node.id);
      pending.push(...inlined.nodes);
      replacement = inlined.edges;
    }

    edges = edges.filter(e => e[0] !== node.id && e[2] !== node.id);
    edges.push(...replacement);
  }

  return { ...graph, nodes, edges };
}

/** A group may be used any number of times, as long as it is not inside itself */
function assertNoGroupCycles(groups: Map<number, GroupDefinition>) {
  const onPath = new Set<number>();
  const checked = new Set<number>();
  function containsCycle(groupId: number): boolean {
    if (onPath.has(groupId)) return true;
    if (checked.has(groupId)) return false;
    onPath.add(groupId);
    for (const n of groups.get(groupId)?.nodes ?? []) {
      const nestedId = groupIdOf(n);
      if (nestedId !== undefined && containsCycle(nestedId)) return true;
    }
    onPath.delete(groupId);
    checked.add(groupId);
    return false;
  }

  for (const group of groups.values()) {
    if (containsCycle(group.id)) {
      throw new Error(`Circular group reference: group ${group.id} contains itself`);
    }
  }
}

/** The edges of a muted instance: its first input passed on to its targets */
function bypassInstance(
  instance: SerializedNode,
  group: GroupDefinition,
  edges: SerializedEdge[]
): SerializedEdge[] {
  const firstKey = groupInputKey(group, 0);
  const incoming = edges.find(e => e[2] === instance.id && e[3] === firstKey);
  if (!incoming) return [];
  return edges
    .filter(e => e[0] === instance.id)
    .map(out => [incoming[0], incoming[1], out[2], out[3]]);
}

type Props = NonNullable<SerializedNode['props']>;

/** The nodes of the group under new ids and the edges that replace those of the instance */
function inlineInstance(
  instance: SerializedNode,
  group: GroupDefinition,
  edges: SerializedEdge[],
  newId: (nodeId: number) => number
): { nodes: SerializedNode[]; edges: SerializedEdge[] } {
  const idMap = new Map<number, number>();

  const inputBoundary = group.nodes.find(n => n.type === GROUP_INPUT);
  const outputBoundary = group.nodes.find(n => n.type === GROUP_OUTPUT);

  const realNodes = group.nodes.filter(
    n => n.type !== GROUP_INPUT && n.type !== GROUP_OUTPUT
  );

  for (const n of realNodes) idMap.set(n.id, newId(n.id));

  const incomingExternal = edges.filter(e => e[2] === instance.id);
  const outgoingExternal = edges.filter(e => e[0] === instance.id);
  const newEdges: SerializedEdge[] = [];

  // external_source → [inputBoundary →] internal_target
  //
  // Match each external edge only to the internal edges that originate from the output
  // socket of the boundary its input stands for, not a cartesian product of all.
  const propOverrides = new Map<number, Props>();
  if (inputBoundary) {
    const fromInput = group.edges
      .filter(e => e[0] === inputBoundary.id)
      .map(e => ({ key: groupInputKey(group, e[1]), to: e[2], socket: e[3] }));
    for (const extEdge of incomingExternal) {
      for (const intEdge of fromInput) {
        if (intEdge.key !== extEdge[3]) continue;
        const toId = idMap.get(intEdge.to);
        if (toId !== undefined) newEdges.push([extEdge[0], extEdge[1], toId, intEdge.socket]);
      }
    }

    const connected = new Set(incomingExternal.map(e => e[3]));
    for (const { key, to, socket } of fromInput) {
      if (key === undefined || connected.has(key)) continue;
      const value = groupInputValue(instance, group, key);
      if (value === undefined) continue;
      propOverrides.set(to, { ...propOverrides.get(to), [socket]: value as Props[string] });
    }
  }

  // internal_source → [outputBoundary →] external_target
  if (outputBoundary) {
    const toOutput = group.edges.filter(e => e[2] === outputBoundary.id);
    for (const extEdge of outgoingExternal) {
      for (const intEdge of toOutput) {
        if (groupOutputIndex(intEdge[3]) !== extEdge[1]) continue;
        const fromId = idMap.get(intEdge[0]);
        if (fromId !== undefined) newEdges.push([fromId, intEdge[1], extEdge[2], extEdge[3]]);
      }
    }
  }

  // internal-to-internal edges (skip boundary edges)
  for (const e of group.edges) {
    if (e[0] === inputBoundary?.id || e[2] === outputBoundary?.id) continue;
    const fromId = idMap.get(e[0]);
    const toId = idMap.get(e[2]);
    if (fromId !== undefined && toId !== undefined) newEdges.push([fromId, e[1], toId, e[3]]);
  }

  const nodes = realNodes.map(n => ({
    ...n,
    id: idMap.get(n.id)!,
    props: propOverrides.has(n.id) ? { ...n.props, ...propOverrides.get(n.id) } : n.props
  }));

  return { nodes, edges: newEdges };
}

/**
 * The graph as it runs, and for every node of it the way to it in the editor: the group nodes to
 * enter from the root, then the node inside the last of them. A node of the root is its own way.
 */
export function expandWithChains(graph: Graph) {
  const sources = new Map<number, NodeSource>();
  const expanded = expandGroups(graph, undefined, sources);
  const chains = new Map<number, number[]>();
  function chainOf(id: number): number[] {
    const source = sources.get(id);
    return source ? [...chainOf(source.instance), source.node] : [id];
  }
  for (const node of expanded.nodes) chains.set(node.id, chainOf(node.id));
  return { graph: expanded, chains };
}
