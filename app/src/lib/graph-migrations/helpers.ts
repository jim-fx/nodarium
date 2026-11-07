import type { Graph, GroupDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';

// Helpers mutate the graph, migrateGraph() hands every migration its own copy

type Scope = { nodes: SerializedNode[]; edges: SerializedEdge[] };

function scopes(graph: Graph): Scope[] {
  return [graph, ...(graph.groups ?? [])];
}

function nodesOfType(scope: Scope, type: string) {
  return new Map(scope.nodes.filter((n) => n.type === type).map((n) => [n.id, n]));
}

export function renameNodeType(graph: Graph, from: string, to: string) {
  for (const { nodes } of scopes(graph)) {
    for (const node of nodes) {
      if (node.type === from) node.type = to as SerializedNode['type'];
    }
  }
  return graph;
}

export function renameInputs(graph: Graph, type: string, renames: Record<string, string>) {
  const rename = (name: string) => renames[name] ?? name;
  for (const scope of scopes(graph)) {
    const nodes = nodesOfType(scope, type);
    if (nodes.size === 0) continue;
    for (const node of nodes.values()) {
      if (node.props) {
        const props: NonNullable<SerializedNode['props']> = {};
        for (const [key, value] of Object.entries(node.props)) {
          if (key in renames && renames[key] in node.props) continue;
          props[rename(key)] = value;
        }
        node.props = props;
      }
      if (node.exposed) node.exposed = [...new Set(node.exposed.map(rename))];
    }
    for (const edge of scope.edges) {
      if (nodes.has(edge[2])) edge[3] = rename(edge[3]);
    }
  }
  return graph;
}

export function mapInput(
  graph: Graph,
  type: string,
  input: string,
  map: (value: number | number[]) => number | number[]
) {
  for (const { nodes } of scopes(graph)) {
    for (const node of nodes) {
      if (node.type !== type || node.props?.[input] === undefined) continue;
      node.props[input] = map(node.props[input]);
    }
  }
  return graph;
}

export function removeInputs(graph: Graph, type: string, inputs: string[]) {
  const removed = new Set(inputs);
  for (const scope of scopes(graph)) {
    const nodes = nodesOfType(scope, type);
    if (nodes.size === 0) continue;
    for (const node of nodes.values()) {
      if (node.props) {
        node.props = Object.fromEntries(
          Object.entries(node.props).filter(([key]) => !removed.has(key))
        );
      }
      if (node.exposed) node.exposed = node.exposed.filter((key) => !removed.has(key));
    }
    scope.edges = scope.edges.filter((edge) => !(nodes.has(edge[2]) && removed.has(edge[3])));
  }
  return graph;
}

export type InputSocket = [nodeId: number, input: string];
export type OutputSocket = [nodeId: number, output: number];

export type Replacement = {
  nodes: SerializedNode[];
  edges?: SerializedEdge[];
  // edges into or out of a socket missing here are dropped
  inputs?: Record<string, InputSocket[]>;
  outputs?: Record<number, OutputSocket>;
};

export type ReplaceContext = {
  nextId(): number;
  addGroup(group: Omit<GroupDefinition, 'id'>): number;
};

export function replaceNodes(
  graph: Graph,
  type: string,
  replace: (node: SerializedNode, context: ReplaceContext) => Replacement | undefined
) {
  graph.groups ??= [];
  const groups = graph.groups;
  // like the editor, ids are unique across the top level, every group and the group ids, so a
  // node can't be mistaken for one of another level (GraphManager.createIdAllocator)
  let lastId = Math.max(
    -1,
    ...scopes(graph).flatMap((scope) => scope.nodes.map((n) => n.id)),
    ...groups.map((g) => g.id)
  );
  const nextId = () => ++lastId;
  const addGroup = (group: Omit<GroupDefinition, 'id'>) => {
    const id = nextId();
    groups.push({ ...group, id });
    return id;
  };
  const context: ReplaceContext = { nextId, addGroup };

  for (const scope of scopes(graph)) {
    const old = nodesOfType(scope, type);
    if (old.size === 0) continue;

    const replacements = new Map<number, Replacement>();
    for (const node of old.values()) {
      const replacement = replace(node, context);
      if (replacement) replacements.set(node.id, replacement);
    }
    if (replacements.size === 0) continue;

    const edges: SerializedEdge[] = [];
    for (const [from, output, to, input] of scope.edges) {
      const sources: OutputSocket[] = replacements.has(from)
        ? [replacements.get(from)!.outputs?.[output]].filter((s) => s !== undefined)
        : [[from, output]];
      const targets: InputSocket[] = replacements.has(to)
        ? (replacements.get(to)!.inputs?.[input] ?? [])
        : [[to, input]];
      for (const source of sources) {
        for (const target of targets) edges.push([source[0], source[1], target[0], target[1]]);
      }
    }
    for (const replacement of replacements.values()) edges.push(...(replacement.edges ?? []));

    scope.nodes = scope.nodes.flatMap((node) => replacements.get(node.id)?.nodes ?? [node]);
    scope.edges = edges;
  }
  return graph;
}
