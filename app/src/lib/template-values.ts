import type { Graph, SerializedNode } from '@nodarium/types';

/** The input values the user changed, by node id. Node ids are unique across the groups. */
export type ChangedValues = Record<number, NonNullable<SerializedNode['props']>>;

function allNodes(graph: Graph): SerializedNode[] {
  return [...graph.nodes, ...(graph.groups ?? []).flatMap((group) => group.nodes)];
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** The values of `edited` that differ from those of `template` */
export function changedValues(template: Graph, edited: Graph): ChangedValues {
  const before = new Map(allNodes(template).map((node) => [node.id, node.props ?? {}]));
  const changed: ChangedValues = {};
  for (const node of allNodes(edited)) {
    const props = before.get(node.id);
    if (!props) continue;
    for (const [key, value] of Object.entries(node.props ?? {})) {
      if (same(value, props[key])) continue;
      (changed[node.id] ??= {})[key] = value;
    }
  }
  return changed;
}

/** A copy of `template` with the changed values, every other value stays as it is */
export function withValues(template: Graph, values: ChangedValues): Graph {
  const graph = structuredClone(template);
  for (const node of allNodes(graph)) {
    const changed = values[node.id];
    if (changed) node.props = { ...node.props, ...structuredClone(changed) };
  }
  return graph;
}

/** The graph the editor opens: the template with the changed values, the seed and the time */
export function editorGraph(
  template: Graph,
  values: ChangedValues,
  { title, seed, time }: { title: string; seed?: number; time?: number }
): Graph {
  const graph = withValues(template, values);
  graph.meta = { ...graph.meta, title };
  graph.settings = {
    ...graph.settings,
    ...(seed !== undefined && { seed }),
    ...(time !== undefined && { 'time.t': time })
  };
  return graph;
}
