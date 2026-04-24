import type {
  Graph,
  NodeDefinition,
  NodeInput,
  NodeRegistry,
  RuntimeExecutor,
  SyncCache
} from '@nodarium/types';

function isGroupInstanceType(type: string): boolean {
  return type === '__virtual/group/instance';
}

export function expandGroups(graph: Graph): Graph {
  if (!graph.groups || Object.keys(graph.groups).length === 0) {
    return graph;
  }

  let nodes = [...graph.nodes];
  let edges = [...graph.edges];
  const groups = graph.groups;

  let changed = true;
  while (changed) {
    changed = false;

    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      if (!isGroupInstanceType(node.type)) continue;

      const groupId = (node.props as Record<string, unknown> | undefined)?.groupId as string | undefined;
      if (!groupId) continue;
      const group = groups[groupId];
      if (!group) continue;

      changed = true;

      // Recursively expand nested groups inside this group's internal graph
      const expandedInternal = expandGroups({
        id: 0,
        nodes: group.graph.nodes,
        edges: group.graph.edges,
        groups
      });

      const ID_PREFIX = node.id * 1000000;
      const idMap = new Map<number, number>();

      const inputVirtualNode = expandedInternal.nodes.find(
        n => n.type === '__virtual/group/input'
      );
      const outputVirtualNode = expandedInternal.nodes.find(
        n => n.type === '__virtual/group/output'
      );

      const realInternalNodes = expandedInternal.nodes.filter(
        n => n.type !== '__virtual/group/input' && n.type !== '__virtual/group/output'
      );

      for (const n of realInternalNodes) {
        idMap.set(n.id, ID_PREFIX + n.id);
      }

      const parentIncomingEdges = edges.filter(e => e[2] === node.id);
      const parentOutgoingEdges = edges.filter(e => e[0] === node.id);

      // Edges from/to virtual nodes in the expanded internal graph
      const edgesFromInput = expandedInternal.edges.filter(
        e => e[0] === inputVirtualNode?.id
      );
      const edgesToOutput = expandedInternal.edges.filter(
        e => e[2] === outputVirtualNode?.id
      );

      const newEdges: Graph['edges'] = [];

      // Short-circuit: parent source → internal target (via group input)
      for (const parentEdge of parentIncomingEdges) {
        const socketName = parentEdge[3];
        const socketIdx = group.inputs.findIndex(s => s.name === socketName);
        if (socketIdx === -1) continue;

        for (const internalEdge of edgesFromInput.filter(e => e[1] === socketIdx)) {
          const remappedId = idMap.get(internalEdge[2]);
          if (remappedId !== undefined) {
            newEdges.push([parentEdge[0], parentEdge[1], remappedId, internalEdge[3]]);
          }
        }
      }

      // Short-circuit: internal source → parent target (via group output)
      for (const parentEdge of parentOutgoingEdges) {
        const outputIdx = parentEdge[1];
        const outputSocketName = group.outputs[outputIdx]?.name;
        if (!outputSocketName) continue;

        for (const internalEdge of edgesToOutput.filter(e => e[3] === outputSocketName)) {
          const remappedId = idMap.get(internalEdge[0]);
          if (remappedId !== undefined) {
            newEdges.push([remappedId, internalEdge[1], parentEdge[2], parentEdge[3]]);
          }
        }
      }

      // Remap internal-to-internal edges
      const internalEdges = expandedInternal.edges.filter(
        e => e[0] !== inputVirtualNode?.id
          && e[0] !== outputVirtualNode?.id
          && e[2] !== inputVirtualNode?.id
          && e[2] !== outputVirtualNode?.id
      );

      for (const e of internalEdges) {
        const fromId = idMap.get(e[0]);
        const toId = idMap.get(e[2]);
        if (fromId !== undefined && toId !== undefined) {
          newEdges.push([fromId, e[1], toId, e[3]]);
        }
      }

      // Remove the group node
      nodes.splice(i, 1);

      // Add remapped internal nodes
      for (const n of realInternalNodes) {
        nodes.push({ ...n, id: idMap.get(n.id)! });
      }

      // Remove group node's edges and add short-circuit edges
      const groupEdgeKeys = new Set([
        ...parentIncomingEdges.map(e => `${e[0]}-${e[1]}-${e[2]}-${e[3]}`),
        ...parentOutgoingEdges.map(e => `${e[0]}-${e[1]}-${e[2]}-${e[3]}`)
      ]);
      edges = edges.filter(
        e => !groupEdgeKeys.has(`${e[0]}-${e[1]}-${e[2]}-${e[3]}`)
      );
      edges.push(...newEdges);

      break; // Restart loop with updated nodes array
    }
  }

  return { ...graph, nodes, edges };
}
import {
  concatEncodedArrays,
  createLogger,
  encodeFloat,
  fastHashArrayBuffer,
  type PerformanceStore
} from '@nodarium/utils';
import type { RuntimeNode } from './types';

const log = createLogger('runtime-executor');
log.mute();

function getValue(input: NodeInput, value?: unknown) {
  if (value === undefined && 'value' in input) {
    value = input.value;
  }

  if (input.type === 'float') {
    return encodeFloat(value as number);
  }

  if (Array.isArray(value)) {
    if (input.type === 'vec3' || input.type === 'shape') {
      return [
        0,
        value.length + 1,
        ...value.map((v) => encodeFloat(v)),
        1,
        1
      ] as number[];
    }
    return [0, value.length + 1, ...value, 1, 1] as number[];
  }

  if (typeof value === 'boolean') {
    return value ? 1 : 0;
  }

  if (typeof value === 'number') {
    return value;
  }

  if (value instanceof Int32Array) {
    return value;
  }

  throw new Error(`Unknown input type ${input.type}`);
}

export class MemoryRuntimeExecutor implements RuntimeExecutor {
  private definitionMap: Map<string, NodeDefinition> = new Map();

  private seed = Math.floor(Math.random() * 100000000);
  private debugData: Record<number, { type: string; data: Int32Array }> = {};

  perf?: PerformanceStore;

  constructor(
    private registry: NodeRegistry,
    public cache?: SyncCache<Int32Array>
  ) {
    this.cache = undefined;
  }

  private async getNodeDefinitions(graph: Graph) {
    if (this.registry.status !== 'ready') {
      throw new Error('Node registry is not ready');
    }

    // Only load non-virtual types (virtual nodes are resolved locally)
    const nonVirtualTypes = graph.nodes
      .map(node => node.type)
      .filter(t => !t.startsWith('__virtual/'));
    await this.registry.load(nonVirtualTypes as any);

    const typeMap = new Map<string, NodeDefinition>();
    for (const node of graph.nodes) {
      if (!typeMap.has(node.type)) {
        const type = this.registry.getNode(node.type);
        if (type) {
          typeMap.set(node.type, type);
        }
      }
    }
    return typeMap;
  }

  private async addMetaData(graph: Graph) {
    // First, lets check if all nodes have a definition
    this.definitionMap = await this.getNodeDefinitions(graph);

    const graphNodes = graph.nodes.map(node => {
      const n = node as RuntimeNode;
      n.state = {
        depth: 0,
        children: [],
        parents: [],
        inputNodes: {}
      };
      return n;
    });

    const outputNode = graphNodes.find((node) => node.type.endsWith('/output'));
    if (!outputNode) {
      throw new Error('No output node found');
    }

    const nodeMap = new Map(
      graphNodes.map((node) => [node.id, node])
    );

    // loop through all edges and assign the parent and child nodes to each node
    for (const edge of graph.edges) {
      const [parentId, /*_parentOutput*/, childId, childInput] = edge;
      const parent = nodeMap.get(parentId);
      const child = nodeMap.get(childId);
      if (parent && child) {
        parent.state.children.push(child);
        child.state.parents.push(parent);
        child.state.inputNodes[childInput] = parent;
      }
    }

    const nodes = new Map<number, RuntimeNode>();

    // loop through all the nodes and assign each nodes its depth
    const stack = [outputNode, ...graphNodes.filter(n => n.type.endsWith('/debug'))];
    while (stack.length) {
      const node = stack.pop();
      if (!node) continue;
      for (const parent of node.state.parents) {
        parent.state = parent.state || {};
        parent.state.depth = node.state.depth + 1;
        stack.push(parent);
      }
      nodes.set(node.id, node);
    }

    for (const node of graphNodes) {
      if (node.type.endsWith('/debug')) {
        node.state = node.state || {};
        const parent = node.state.parents[0];
        if (parent) {
          node.state.depth = parent.state.depth - 1;
          parent.state.debugNode = true;
        }
        nodes.set(node.id, node);
      }
    }

    const _nodes = [...nodes.values()];

    return [outputNode, _nodes] as const;
  }

  async execute(graph: Graph, settings: Record<string, unknown>) {
    this.perf?.addPoint('runtime');

    let a = performance.now();
    this.debugData = {};

    // Expand group nodes into a flat graph before execution
    graph = expandGroups(graph);

    // Then we add some metadata to the graph
    const [outputNode, nodes] = await this.addMetaData(graph);
    let b = performance.now();

    this.perf?.addPoint('collect-metadata', b - a);

    /*
     * Here we sort the nodes into buckets, which we then execute one by one
     * +-b2-+-b1-+---b0---+
     * |    |    |        |
     * | n3 | n2 | Output |
     * | n6 | n4 | Level  |
     * |    | n5 |        |
     * |    |    |        |
     * +----+----+--------+
     */

    // we execute the nodes from the bottom up
    const sortedNodes = nodes.sort(
      (a, b) => (b.state?.depth || 0) - (a.state?.depth || 0)
    );

    // here we store the intermediate results of the nodes
    const results: Record<string, Int32Array> = {};

    if (settings['randomSeed']) {
      this.seed = Math.floor(Math.random() * 100000000);
    }

    for (const node of sortedNodes) {
      const node_type = this.definitionMap.get(node.type)!;

      if (!node_type || !node.state || !node_type.execute) {
        log.warn(`Node ${node.id} has no definition`);
        continue;
      }

      a = performance.now();

      // Collect the inputs for the node
      const inputs = Object.entries(node_type.inputs || {}).map(
        ([key, input]) => {
          if (input.type === 'seed') {
            return this.seed;
          }

          // If the input is linked to a setting, we use that value
          if (input.setting) {
            return getValue(input, settings[input.setting]);
          }

          // check if the input is connected to another node
          const inputNode = node.state.inputNodes[key];
          if (inputNode) {
            if (results[inputNode.id] === undefined) {
              throw new Error(
                `Node ${node.type} is missing input from node ${inputNode.type}`
              );
            }
            return results[inputNode.id];
          }

          // If the value is stored in the node itself, we use that value
          if (node.props?.[key] !== undefined) {
            return getValue(input, node.props[key]);
          }

          return getValue(input);
        }
      );
      b = performance.now();

      this.perf?.addPoint('collected-inputs', b - a);

      try {
        a = performance.now();
        const encoded_inputs = concatEncodedArrays(inputs);
        b = performance.now();
        this.perf?.addPoint('encoded-inputs', b - a);

        a = performance.now();
        const inputHash = `node-${node.id}-${fastHashArrayBuffer(encoded_inputs)}`;
        b = performance.now();
        this.perf?.addPoint('hash-inputs', b - a);

        const cachedValue = this.cache?.get(inputHash);
        if (cachedValue !== undefined) {
          log.log(`Using cached value for ${node_type.id || node.id}`);
          this.perf?.addPoint('cache-hit', 1);
          results[node.id] = cachedValue as Int32Array;
          if (node.state.debugNode && node_type.outputs) {
            this.debugData[node.id] = {
              type: node_type.outputs[0],
              data: cachedValue
            };
          }
          continue;
        }
        this.perf?.addPoint('cache-hit', 0);

        log.group(`executing ${node_type.id}-${node.id}`);
        log.log(`Inputs:`, inputs);
        a = performance.now();
        results[node.id] = node_type.execute(encoded_inputs);
        if (node.state.debugNode && node_type.outputs) {
          this.debugData[node.id] = {
            type: node_type.outputs[0],
            data: results[node.id]
          };
        }
        log.log('Executed', node.type, node.id);
        b = performance.now();

        if (this.cache && node.id !== outputNode.id) {
          this.cache.set(inputHash, results[node.id]);
        }

        this.perf?.addPoint('node/' + node_type.id, b - a);
        log.log('Result:', results[node.id]);
        log.groupEnd();
      } catch (e) {
        log.groupEnd();
        log.error(`Error executing node ${node_type.id || node.id}`, e);
      }
    }

    // return the result of the parent of the output node
    const res = results[outputNode.id];

    if (this.cache) {
      this.cache.size = sortedNodes.length * 2;
    }

    this.perf?.endPoint('runtime');

    return res as unknown as Int32Array;
  }

  getDebugData() {
    return this.debugData;
  }

  getPerformanceData() {
    return this.perf?.get();
  }
}
