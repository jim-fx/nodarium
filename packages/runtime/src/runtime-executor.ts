import type {
  Graph,
  NodeDefinition,
  NodeInput,
  NodeRegistry,
  NodeValue,
  RuntimeExecutor,
  SyncCache
} from '@nodarium/types';
import { areSocketsCompatible, getOutputType, INTERNAL_PREFIX } from '@nodarium/types';
import {
  createLogger,
  decodeFloat,
  encodeFloat,
  hashString53,
  type PerformanceStore,
  readNodeValue
} from '@nodarium/utils';
import { startCacheRun } from './cache-run';
import { getErrorInfo, NodeExecutionError, type RuntimeErrorInfo } from './errors';
import { expandGroups } from './expand-groups';
import { migrateGraph } from './graph-migrations';
import { InstanceTracker } from './instances';
import type { DebugEntry, RuntimeNode } from './types';

/** The node that turns a value of this socket type into something the debug view can draw */
const DEBUG_PREVIEWS: Record<string, { node: string }> = {
  // a flat disc colored by the pattern
  pattern: { node: 'max/myzel/output' }
};
const log = createLogger('runtime-executor');
log.mute();

/**
 * The id a node writes into the paths it makes, so every path knows which node made it.
 * Node ids can be larger than 32 bits (nodes of a group instance), the wasm side reads an i32.
 */
export function toCreatorId(nodeId: number): number {
  return (nodeId | 0) ^ (Math.floor(nodeId / 2 ** 32) | 0);
}

function getValue(input: NodeInput, value?: unknown) {
  if (value === undefined && 'value' in input) {
    value = input.value;
  }

  if (input.type === 'float') {
    return encodeFloat(value as number);
  }

  if (input.type === 'select' && typeof value !== 'number') {
    const index = input.options?.indexOf(value as string);
    if (index === undefined || index < 0) {
      // Default to the first option
      return 0;
    }
    return index;
  }

  if (Array.isArray(value)) {
    if (input.type === 'vec3' || input.type === 'shape' || input.type === 'curve') {
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

  // an unconnected input without a default, like the geometry of the output node
  if (value === undefined) {
    return [0, 1, 1, 1];
  }

  throw new Error(`Unknown input type ${input.type}`);
}

export class MemoryRuntimeExecutor implements RuntimeExecutor {
  private definitionMap: Map<string, NodeDefinition> = new Map();
  private instances = new InstanceTracker();

  // keyed by the id of the node a debug node is connected to
  private debugData: Record<number, DebugEntry> = {};
  // execution time in ms of every node of the last run, cached nodes keep
  // the time of the run that computed their result, group nodes the sum of their nodes
  private nodeTimings: Record<number, number> = {};
  // the same, but only of the nodes that ran in the last run and not from the cache
  private runTimings: Record<number, number> = {};
  private lastError: RuntimeErrorInfo | undefined;

  perf?: PerformanceStore;

  constructor(
    private registry: NodeRegistry,
    public cache?: SyncCache<Int32Array[]>
  ) {}

  /** Bytes of memory a node may keep after a run, see {@link RELEASE_INSTANCES_OVER} */
  get releaseInstancesOver() {
    return this.instances.releaseOver;
  }

  set releaseInstancesOver(bytes: number) {
    this.instances.releaseOver = bytes;
  }

  private async getNodeDefinitions(graph: Graph) {
    if (this.registry.status !== 'ready') {
      throw new Error('Node registry is not ready');
    }

    // Only load non-virtual types (virtual nodes are resolved locally)
    const nonVirtualTypes = graph.nodes
      .map(node => node.type)
      .filter(t => !t.startsWith(INTERNAL_PREFIX));
    await this.registry.load(nonVirtualTypes);

    const typeMap = new Map<string, NodeDefinition>();
    for (const node of graph.nodes) {
      if (!typeMap.has(node.type)) {
        const type = this.registry.getNode(node.type);
        if (type) typeMap.set(node.type, type);
      }
    }
    return typeMap;
  }

  private async addMetaData(graph: Graph) {
    this.definitionMap = await this.getNodeDefinitions(graph);

    let graphNodes = graph.nodes.map(node => {
      const n = node as RuntimeNode;
      if (!n) return null;
      n.state = {
        depth: 0,
        children: [],
        parents: [],
        inputNodes: {},
        inputOutputs: {}
      };
      return n;
    });

    const outputNode = graphNodes.find((node) => node?.type.endsWith('/output'));
    if (!outputNode) {
      throw new Error('No output node found');
    }

    const nodeMap = new Map(
      graphNodes.filter(n => n !== null).map((node) => [node.id, node])
    );

    for (const edge of graph.edges) {
      const [parentId, parentOutput, childId, childInput] = edge;
      const parent = nodeMap.get(parentId);
      const child = nodeMap.get(childId);
      if (parent && child) {
        parent.state.children.push(child);
        child.state.parents.push(parent);
        child.state.inputNodes[childInput] = parent;
        child.state.inputOutputs[childInput] = parentOutput;
      }
    }

    // Remove all muted nodes from the graph, a muted node passes the node
    // connected to its first input through to its children
    const isBypassed = (node: NonNullable<RuntimeNode>) =>
      node.muted === true && !node.type.endsWith('/debug') && !node.type.endsWith('/output');

    type LiveNode = NonNullable<RuntimeNode>;
    const resolveSource = (
      node: LiveNode,
      output: number
    ): { node: LiveNode; output: number } | undefined => {
      const seen = new Set<number>();
      let current: LiveNode | undefined = node;
      while (current && isBypassed(current)) {
        if (seen.has(current.id)) return undefined;
        seen.add(current.id);
        const inputs: Record<string, unknown> = this.definitionMap.get(current.type)?.inputs || {};
        const firstInput: string | undefined = Object.keys(inputs)[0];
        output = firstInput ? current.state.inputOutputs[firstInput] ?? 0 : 0;
        current = firstInput
          ? current.state.inputNodes[firstInput] as LiveNode | undefined
          : undefined;
      }
      return current && { node: current, output };
    };

    graphNodes = graphNodes.filter(n => n && !isBypassed(n));
    for (const node of graphNodes) {
      if (!node) continue;
      node.state.children = [];
    }
    for (const node of graphNodes) {
      if (!node) continue;
      for (const [key, inputNode] of Object.entries(node.state.inputNodes)) {
        const source = resolveSource(
          inputNode as NonNullable<RuntimeNode>,
          node.state.inputOutputs[key] ?? 0
        );
        // a bypassed value is dropped if the child cannot accept its type
        const sourceType = source
          && getOutputType(this.definitionMap.get(source.node.type)?.outputs?.[source.output]);
        const input = this.definitionMap.get(node.type)?.inputs?.[key];
        const compatible = source?.node === inputNode
          || (!!input
            && areSocketsCompatible(sourceType as string, [input.type, ...(input.accepts || [])]));
        if (source && compatible) {
          node.state.inputNodes[key] = source.node;
          node.state.inputOutputs[key] = source.output;
          source.node.state.children.push(node);
        } else {
          delete node.state.inputNodes[key];
          delete node.state.inputOutputs[key];
        }
      }
      node.state.parents = Object.values(node.state.inputNodes);
    }

    const nodes = new Map<number, RuntimeNode>();

    const stack = [outputNode, ...graphNodes.filter(n => n?.type.endsWith('/debug'))];
    while (stack.length) {
      const node = stack.pop();
      if (!node) continue;
      for (const parent of node.state.parents) {
        if (!parent) continue;
        parent.state = parent.state || {};
        // a node that feeds several nodes has to run before the deepest of them
        if (parent.state.depth > node.state.depth) continue;
        parent.state.depth = node.state.depth + 1;
        stack.push(parent);
      }
      nodes.set(node.id, node);
    }

    for (const node of graphNodes) {
      if (!node) continue;
      if (node.type.endsWith('/debug')) {
        node.state = node.state || {};
        const parent = node.state.inputNodes['input'];
        if (parent) {
          // after the input it shows and the nodes an exposed amount or clip comes from
          node.state.depth = Math.min(...node.state.parents.map((p) =>
            p?.state.depth ?? parent.state.depth
          )) - 1;
          parent.state.debugOutput = node.state.inputOutputs['input'] ?? 0;
        }
        nodes.set(node.id, node);
      }
    }

    const _nodes = [...nodes.values()];

    return [outputNode, _nodes] as const;
  }

  async execute(graph: Graph, settings: Record<string, unknown>) {
    this.lastError = undefined;
    this.instances.enterRun();
    try {
      return await this.executeGraph(graph, settings);
    } catch (error) {
      this.lastError = getErrorInfo(error);
      throw error;
    } finally {
      this.instances.leaveRun();
    }
  }

  /** Releases the instances of all nodes that ran, once no run is going any more */
  releaseInstances() {
    this.instances.releaseAll();
  }

  private async executeGraph(graph: Graph, settings: Record<string, unknown>) {
    this.perf?.addPoint('runtime');

    let a = performance.now();
    this.debugData = {};

    graph = migrateGraph(graph);

    const origins = new Map<number, number>();
    graph = expandGroups(graph, origins);

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

    const sortedNodes = nodes.sort(
      (a, b) => (b?.state?.depth || 0) - (a?.state?.depth || 0)
    );

    // here we store the intermediate results of the nodes, results of wasm
    // nodes stay inside their module's memory until the next run
    const results: Record<string, NodeValue[]> = {};
    // structural cache keys: a node's key is derived from its own input
    // values and the keys of its input nodes, so we never hash result data
    const cacheKeys: Record<string, number> = {};
    const nodeTimings: Record<number, number> = {};
    const runTimings: Record<number, number> = {};

    const seed = typeof settings['seed'] === 'number' && Number.isFinite(settings['seed'])
      ? Math.floor(settings['seed'])
      : 0;

    // nothing below this may await
    this.instances.resetFor(this.definitionMap);
    const cache = startCacheRun(this.cache, sortedNodes.length);

    type PlannedInput = Int32Array | { node: NonNullable<RuntimeNode>; output: number };
    const planned: {
      node: NonNullable<RuntimeNode>;
      type: NodeDefinition;
      inputs: PlannedInput[];
      // inputs that fell back to an empty value, nodes may panic on these
      missingInputs: string[];
      inputHash?: string;
    }[] = [];

    for (const node of sortedNodes) {
      if (!node) continue;
      const node_type = this.definitionMap.get(node.type)!;

      if (!node_type || !node.state || !node_type.execute) {
        log.warn(`Node ${node.id} has no definition`);
        continue;
      }

      a = performance.now();

      const keyParts: (string | number)[] = [node.type];
      const missingInputs: string[] = [];

      const inputs = Object.entries(node_type.inputs || {}).map(
        ([key, input]): PlannedInput => {
          const inputNode = !input.setting && input.type !== 'seed' && input.type !== 'creator'
            ? node.state.inputNodes[key]
            : undefined;
          if (inputNode) {
            const output = node.state.inputOutputs[key] ?? 0;
            keyParts.push(
              output === 0 ? cacheKeys[inputNode.id] : `${cacheKeys[inputNode.id]}:${output}`
            );
            return { node: inputNode, output };
          }

          let value: number | number[] | Int32Array;
          if (input.type === 'seed') {
            value = seed;
          } else if (input.type === 'creator') {
            value = toCreatorId(node.id);
          } else if (input.setting) {
            value = getValue(input, settings[input.setting]);
          } else if (node.props?.[key] !== undefined) {
            value = getValue(input, node.props[key]);
          } else {
            if (!('value' in input) || input.value === undefined) missingInputs.push(key);
            value = getValue(input);
          }

          if (typeof value === 'number') {
            keyParts.push('n' + value);
            return Int32Array.of(value);
          }
          keyParts.push('a' + value.join(','));
          return value instanceof Int32Array ? value : Int32Array.from(value);
        }
      );
      b = performance.now();

      this.perf?.addPoint('collected-inputs', b - a);

      let inputHash: string | undefined;
      if (cache.enabled) {
        a = performance.now();
        cacheKeys[node.id] = hashString53(keyParts.join('|'));
        inputHash = `node-${node.id}-${cacheKeys[node.id]}`;
        b = performance.now();
        this.perf?.addPoint('hash-inputs', b - a);
      }

      planned.push({ node, type: node_type, inputs, missingInputs, inputHash });
    }

    // From the output back: a node runs when the cache doesn't have its result and a node that
    // runs, or a debug node, needs it. So a value the cache doesn't keep is only made again when
    // something below it changed. Every entry of the graph is read, they all belong to this run.
    const isDebug = (node: NonNullable<RuntimeNode>) => node.type.endsWith('/debug');
    const needed = new Set<number>([outputNode.id]);
    const cached = new Set<number>();
    for (let i = planned.length - 1; i >= 0; i--) {
      const { node, inputs, inputHash } = planned[i];
      const cachedValue = inputHash !== undefined && node.id !== outputNode.id
        ? cache.read(inputHash)
        : undefined;
      if (cachedValue !== undefined) {
        results[node.id] = cachedValue;
        cached.add(node.id);
      }
      if (isDebug(node)) needed.add(node.id);
      if (!needed.has(node.id)) continue;
      // a debug node shows its inputs, and the amount and clip from them
      if (cachedValue !== undefined && !isDebug(node)) continue;
      for (const input of inputs) if (!(input instanceof Int32Array)) needed.add(input.node.id);
    }

    for (
      const { node, type: node_type, inputs: plannedInputs, missingInputs, inputHash } of planned
    ) {
      const nodeError = (cause: unknown) =>
        new NodeExecutionError(
          cause,
          { id: origins.get(node.id) ?? node.id, type: node.type },
          node_type,
          missingInputs
        );

      if (!needed.has(node.id) || cached.has(node.id)) {
        if (needed.has(node.id)) this.perf?.addPoint('cache-hit', 1);
        if (node.id in this.nodeTimings) nodeTimings[node.id] = this.nodeTimings[node.id];
      }
      if (!needed.has(node.id)) continue;

      const keys = Object.keys(node_type.inputs || {});
      const inputs = !cached.has(node.id) || isDebug(node)
        ? plannedInputs.map((input, index): NodeValue => {
          if (input instanceof Int32Array) return input;
          const value = results[input.node.id]?.[input.output];
          if (value === undefined) {
            throw nodeError(
              `input "${
                keys[index]
              }" did not receive output ${input.output} from ${input.node.type}#${input.node.id}`
            );
          }
          return value;
        })
        : [];

      if (isDebug(node)) this.storeDebugView(node, node_type, inputs);

      if (cached.has(node.id)) {
        log.log(`Using cached value for ${node_type.id || node.id}`);
        this.storeDebugData(node, node_type, results[node.id]);
        continue;
      }
      this.perf?.addPoint('cache-hit', 0);

      try {
        log.group(`executing ${node_type.id}-${node.id}`);
        log.log(`Inputs:`, inputs);
        // a module that has to be instantiated first takes longer than it will the next time
        const cold = node_type.memoryBytes?.() === 0;
        a = performance.now();
        const result = node_type.execute!(inputs);
        results[node.id] = Array.isArray(result) ? result : [result];
        this.storeDebugData(node, node_type, results[node.id]);
        log.log('Executed', node.type, node.id);
        b = performance.now();

        if (inputHash && node.id !== outputNode.id) {
          const cost = cold && node.id in this.nodeTimings
            ? Math.min(b - a, this.nodeTimings[node.id])
            : b - a;
          cache.store(inputHash, results[node.id], cost);
        }

        nodeTimings[node.id] = b - a;
        runTimings[node.id] = b - a;
        this.perf?.addPoint('node/' + node_type.id, b - a);
        log.log('Result:', results[node.id]);
        log.groupEnd();
      } catch (e) {
        log.groupEnd();
        throw nodeError(e);
      }
    }

    // return the result of the parent of the output node
    const outputResult = results[outputNode.id]?.[0];
    const res = outputResult && readNodeValue(outputResult);

    // after the result is copied, a preview may run the output node once more
    this.previewDebugData();

    cache.finish();

    // the nodes of a group run under ids the editor doesn't know, their group node shows the sum
    for (const timings of [nodeTimings, runTimings]) {
      for (const [id, origin] of origins) {
        if (id in timings) timings[origin] = (timings[origin] ?? 0) + timings[id];
      }
    }
    this.nodeTimings = nodeTimings;
    this.runTimings = runTimings;

    this.perf?.endPoint('runtime');

    return res as unknown as Int32Array;
  }

  private storeDebugData(
    node: NonNullable<RuntimeNode>,
    type: NodeDefinition,
    values: NodeValue[]
  ) {
    const output = node.state.debugOutput;
    if (output === undefined || !type.outputs) return;
    const value = values[output];
    if (value === undefined) return;
    this.debugData[node.id] = {
      type: getOutputType(type.outputs[output]) ?? 'unknown',
      output,
      data: readNodeValue(value)
    };
  }

  /**
   * The amount and clip of a debug node go along with the data it shows, the viewer applies
   * them. So the output of the debugged node and every cache key stay as they are.
   */
  private storeDebugView(
    node: NonNullable<RuntimeNode>,
    type: NodeDefinition,
    inputs: NodeValue[]
  ) {
    const source = node.state.inputNodes['input'];
    const entry = source && this.debugData[source.id];
    if (!entry) return;
    const keys = Object.keys(type.inputs ?? {});
    for (const key of ['amount', 'clip'] as const) {
      const value = inputs[keys.indexOf(key)];
      if (value !== undefined) entry[key] = decodeFloat(readNodeValue(value)[0]);
    }
  }

  /**
   * Values the viewer can't draw as they are get turned into something it can, by a node of the
   * graph's own collection. A pattern is only numbers, its preview is the mesh of the output node.
   */
  private previewDebugData() {
    for (const entry of Object.values(this.debugData)) {
      const preview = DEBUG_PREVIEWS[entry.type];
      const node = preview && this.definitionMap.get(preview.node);
      if (!node) continue;
      try {
        const result = node.execute([entry.data]);
        entry.data = readNodeValue(Array.isArray(result) ? result[0] : result);
      } catch (e) {
        log.warn(`could not preview the ${entry.type} for debugging`, e);
      }
    }
  }

  getDebugData() {
    return this.debugData;
  }

  getPerformanceData() {
    return this.perf?.get();
  }

  getNodeTimings() {
    return this.nodeTimings;
  }

  getRunTimings() {
    return this.runTimings;
  }

  /** Details of the error of the last run, if it failed */
  getLastError() {
    return this.lastError;
  }
}
