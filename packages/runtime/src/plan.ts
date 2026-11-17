import type { Graph, NodeDefinition, SerializedNode } from '@nodarium/types';
import { getOutputType, inputAccepts, isDebugNode, isOutputNode } from '@nodarium/types';
import { createLogger, hashString53, type PerformanceStore } from '@nodarium/utils';
import { GraphCycleError } from './errors';
import { type InputContext, resolveInput } from './inputs';

const log = createLogger('runtime-plan');
log.mute();

export type PlanNode = {
  id: number;
  type: string;
  kind: 'node' | 'debug' | 'output';
  definition: NodeDefinition;
  /** one per input of the definition, in its order */
  inputs: PlannedInput[];
  /** inputs that fell back to an empty value */
  missingInputs: string[];
  /** only with a cache */
  cacheKey?: number;
  /** the output a debug node shows, if one is connected */
  debugOutput?: number;
};

/** A node an input reads from, nodes without a definition are not in the plan but can be read */
export type PlanSource = Pick<PlanNode, 'id' | 'type' | 'cacheKey'>;

export type PlannedInput = Int32Array | { node: PlanSource; output: number };

export type Plan = {
  output: PlanSource;
  /** deepest first, the order they run in */
  nodes: PlanNode[];
};

type Connection = { node: LinkedNode; output: number };

/** A node of the graph while the plan is built */
type LinkedNode = {
  id: number;
  type: string;
  kind: PlanNode['kind'];
  props: SerializedNode['props'];
  muted: boolean;
  definition: NodeDefinition | undefined;
  depth: number;
  /** by the key of the input */
  connections: Record<string, Connection>;
  parents: LinkedNode[];
  debugOutput?: number;
};

/**
 * Which nodes of a graph run, in which order and with which inputs. The graph is read, not
 * written, it has to be migrated and its groups expanded.
 */
export function buildPlan(
  graph: Graph,
  definitions: Map<string, NodeDefinition>,
  settings: Record<string, unknown>,
  options: { cacheKeys: boolean; perf?: PerformanceStore; startedAt?: number }
): Plan {
  const { perf, startedAt = performance.now() } = options;
  const linked = linkNodes(graph, definitions);
  const output = linked.find(n => n.kind === 'output');
  if (!output) {
    throw new Error('No output node found');
  }
  const ordered = orderByDepth(output, bypassMuted(linked));
  perf?.addPoint('collect-metadata', performance.now() - startedAt);

  const seed = typeof settings['seed'] === 'number' && Number.isFinite(settings['seed'])
    ? Math.floor(settings['seed'])
    : 0;

  const planned = new Map<LinkedNode, PlanNode>();
  const sourceOf = (node: LinkedNode): PlanSource =>
    planned.get(node) ?? { id: node.id, type: node.type };
  const nodes: PlanNode[] = [];
  for (const node of ordered) {
    const definition = node.definition;
    if (!definition?.execute) {
      log.warn(`Node ${node.id} has no definition`);
      continue;
    }

    let a = performance.now();
    const ctx: InputContext = { seed, settings, keyParts: [node.type], missingInputs: [] };
    const inputs = Object.entries(definition.inputs || {}).map(([key, input]) => {
      const connection = node.connections[key];
      const source = connection && { node: sourceOf(connection.node), output: connection.output };
      return resolveInput(node, key, input, source, ctx);
    });
    let b = performance.now();
    perf?.addPoint('collected-inputs', b - a);

    const planNode: PlanNode = {
      id: node.id,
      type: node.type,
      kind: node.kind,
      definition,
      inputs,
      missingInputs: ctx.missingInputs,
      debugOutput: node.debugOutput
    };
    // a structural key: from the input values and the keys of the input nodes, never the results
    if (options.cacheKeys) {
      a = performance.now();
      planNode.cacheKey = hashString53(ctx.keyParts.join('|'));
      b = performance.now();
      perf?.addPoint('hash-inputs', b - a);
    }
    planned.set(node, planNode);
    nodes.push(planNode);
  }

  return { output: sourceOf(output), nodes };
}

function linkNodes(graph: Graph, definitions: Map<string, NodeDefinition>): LinkedNode[] {
  const linked = graph.nodes.map((node): LinkedNode => ({
    id: node.id,
    type: node.type,
    kind: isOutputNode(node.type) ? 'output' : isDebugNode(node.type) ? 'debug' : 'node',
    props: node.props,
    muted: node.muted === true,
    definition: definitions.get(node.type),
    depth: 0,
    connections: {},
    parents: []
  }));
  const byId = new Map(linked.map(node => [node.id, node]));

  for (const [parentId, output, childId, key] of graph.edges) {
    const parent = byId.get(parentId);
    const child = byId.get(childId);
    if (parent && child) child.connections[key] = { node: parent, output };
  }
  return linked;
}

/**
 * Leaves out the muted nodes, a muted node passes the node connected to its first input through
 * to its children. A passed value is dropped if the child cannot accept its type.
 */
function bypassMuted(nodes: LinkedNode[]): LinkedNode[] {
  const isBypassed = (node: LinkedNode) => node.muted && node.kind === 'node';

  const resolveSource = (connection: Connection): Connection | undefined => {
    const seen = new Set<LinkedNode>();
    let { node: current, output } = connection;
    while (isBypassed(current)) {
      if (seen.has(current)) return undefined;
      seen.add(current);
      const firstInput: string | undefined = Object.keys(current.definition?.inputs || {})[0];
      const next: Connection | undefined = firstInput ? current.connections[firstInput] : undefined;
      if (!next) return undefined;
      current = next.node;
      output = next.output;
    }
    return { node: current, output };
  };

  const live = nodes.filter(node => !isBypassed(node));
  for (const node of live) {
    const connections: Record<string, Connection> = {};
    for (const [key, connection] of Object.entries(node.connections)) {
      const source = resolveSource(connection);
      if (!source) continue;
      if (source.node !== connection.node) {
        const input = node.definition?.inputs?.[key];
        const sourceType = getOutputType(source.node.definition?.outputs?.[source.output]);
        if (!input || !inputAccepts(input, sourceType)) continue;
      }
      connections[key] = source;
    }
    node.connections = connections;
    node.parents = Object.values(connections).map(c => c.node);
  }
  return live;
}

/**
 * The nodes the output and the debug nodes need, deepest first. A node that feeds several nodes
 * runs before the deepest of them.
 */
function orderByDepth(output: LinkedNode, nodes: LinkedNode[]): LinkedNode[] {
  const debugNodes = nodes.filter(n => n.kind === 'debug');
  const reached = new Map<number, LinkedNode>();

  const roots = [output, ...debugNodes];
  const stack = [...roots];
  while (stack.length) {
    const node = stack.pop()!;
    for (const parent of node.parents) {
      if (parent.depth > node.depth) continue;
      parent.depth = node.depth + 1;
      // no path without a cycle is longer than the number of nodes
      if (parent.depth >= nodes.length) throw cycleError(roots);
      stack.push(parent);
    }
    reached.set(node.id, node);
  }

  for (const node of debugNodes) {
    const shown = node.connections['input'];
    if (!shown) continue;
    // after the input it shows and the nodes an exposed amount or clip comes from
    node.depth = Math.min(...node.parents.map(p => p.depth)) - 1;
    shown.node.debugOutput = shown.output;
  }

  return [...reached.values()].sort((a, b) => b.depth - a.depth);
}

function cycleError(roots: LinkedNode[]) {
  const path: LinkedNode[] = [];
  const done = new Set<LinkedNode>();
  const findCycle = (node: LinkedNode): LinkedNode[] | undefined => {
    const index = path.indexOf(node);
    if (index !== -1) return [...path.slice(index), node];
    if (done.has(node)) return undefined;
    path.push(node);
    for (const parent of node.parents) {
      const cycle = findCycle(parent);
      if (cycle) return cycle;
    }
    path.pop();
    done.add(node);
    return undefined;
  };
  for (const root of roots) {
    const cycle = findCycle(root);
    // the path runs against the edges, from a node to the nodes it reads from
    if (cycle) return new GraphCycleError(cycle.reverse().map(n => ({ id: n.id, type: n.type })));
  }
  return new GraphCycleError([]);
}
