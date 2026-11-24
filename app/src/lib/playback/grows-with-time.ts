import type { Graph } from '@nodarium/types';

const TIME = 'max/core/time';
const OUTPUT = 'max/core/output';

/** Whether a time node is upstream of the output, of the graph or in a group it uses */
export function growsWithTime(graph: Graph): boolean {
  const groups = new Map((graph.groups ?? []).map((g) => [g.id, g]));
  const seenGroups = new Set<number>();

  function groupHasTime(id: number): boolean {
    const group = groups.get(id);
    if (!group || seenGroups.has(id)) return false;
    seenGroups.add(id);
    return group.nodes.some((node) =>
      node.type === TIME
        ? !node.muted && group.edges.some(([from]) => from === node.id)
        : groupHasTime(node.props?.groupId as number)
    );
  }

  const nodes = new Map(graph.nodes.map((n) => [n.id, n]));
  const reached = graph.nodes.filter((n) => n.type === OUTPUT).map((n) => n.id);
  // the array grows while it is walked
  for (const id of reached) {
    const node = nodes.get(id);
    if (node?.type === TIME && !node.muted) return true;
    if (groupHasTime(node?.props?.groupId as number)) return true;
    for (const [from, , to] of graph.edges) {
      if (to === id && !reached.includes(from)) reached.push(from);
    }
  }
  return false;
}

/** The time a graph is shown at, its growth ends there. A graph shown at 0 grows to the end. */
export function grownTime(graph: Graph): number {
  const t = graph.settings?.['time.t'];
  return typeof t === 'number' && t > 0 ? Math.min(t, 1) : 1;
}
