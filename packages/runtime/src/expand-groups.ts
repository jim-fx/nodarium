import type { Graph, GroupDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { GROUP_INPUT, GROUP_INSTANCE, GROUP_OUTPUT } from '@nodarium/types';
import { hashString53 } from '@nodarium/utils';

/**
 * @param origins filled with the ID of the outermost group node every expanded
 * node comes from, so errors can point at a node that is visible in the editor
 */
export function expandGroups(graph: Graph, origins?: Map<number, number>): Graph {
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
      const inlined = inlineInstance(node, group, edges, id => createExpandedId(node.id, id));
      for (const n of inlined.nodes) origins?.set(n.id, origins.get(node.id) ?? node.id);
      pending.push(...inlined.nodes);
      replacement = inlined.edges;
    }

    edges = edges.filter(e => e[0] !== node.id && e[2] !== node.id);
    edges.push(...replacement);
  }

  return { ...graph, nodes, edges };
}

function assertNoGroupCycles(groups: Map<number, GroupDefinition>) {
  function groupContainsSelf(groupId: number, visited = new Set<number>()): boolean {
    if (visited.has(groupId)) return true;
    visited.add(groupId);
    const group = groups.get(groupId);
    if (!group) return false;
    for (const n of group.nodes) {
      if (n.type === GROUP_INSTANCE) {
        const nestedId = n.props?.groupId as number | undefined;
        if (nestedId !== undefined && groupContainsSelf(nestedId, visited)) return true;
      }
    }
    return false;
  }

  for (const group of groups.values()) {
    if (groupContainsSelf(group.id)) {
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
  const firstKey = Object.keys(group.inputs ?? {})[0];
  const incoming = edges.find(e => e[2] === instance.id && e[3] === firstKey);
  if (!incoming) return [];
  return edges
    .filter(e => e[0] === instance.id)
    .map(out => [incoming[0], incoming[1], out[2], out[3]]);
}

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
  // The input boundary's sockets are positional: output index N of the
  // boundary belongs to the N-th key of group.inputs (keys can have gaps
  // after a removal). Match each external edge only to the internal edges
  // that originate from that specific output slot — not a cartesian product of all.
  const propOverrides = new Map<number, NonNullable<SerializedNode['props']>>();
  if (inputBoundary) {
    const inputKeys = Object.keys(group.inputs ?? {});
    const fromInput = group.edges.filter(e => e[0] === inputBoundary.id);
    for (const extEdge of incomingExternal) {
      const inputIndex = inputKeys.indexOf(extEdge[3]);
      const matchingIntEdges = fromInput.filter(e => e[1] === inputIndex);
      for (const intEdge of matchingIntEdges) {
        const toId = idMap.get(intEdge[2]);
        if (toId !== undefined) newEdges.push([extEdge[0], extEdge[1], toId, intEdge[3]]);
      }
    }

    // Unconnected group inputs pass the value set on the group node through,
    // or the group input's default, which the group node's widget shows
    const connected = new Set(incomingExternal.map(e => e[3]));
    for (const intEdge of fromInput) {
      const key = inputKeys[intEdge[1]];
      const value = instance.props?.[key] ?? group.inputs?.[key]?.value;
      if (key === undefined || connected.has(key) || value === undefined) continue;
      propOverrides.set(intEdge[2], { ...propOverrides.get(intEdge[2]), [intEdge[3]]: value });
    }
  }

  // internal_source → [outputBoundary →] external_target
  // Group output socket N is the "out_N" input of the output boundary
  // (older groups used "Out" for their single output).
  if (outputBoundary) {
    const toOutput = group.edges.filter(e => e[2] === outputBoundary.id);
    for (const extEdge of outgoingExternal) {
      for (const intEdge of toOutput) {
        if (Number(/^out_(\d+)$/.exec(intEdge[3])?.[1] ?? 0) !== extEdge[1]) continue;
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
