import type {
  Graph,
  NodeDefinition,
  NodeRegistry,
  NodeValue,
  RuntimeExecutor,
  SyncCache
} from '@nodarium/types';
import { getOutputType, INTERNAL_PREFIX } from '@nodarium/types';
import { createLogger, decodeFloat, type PerformanceStore, readNodeValue } from '@nodarium/utils';
import { type CacheRun, startCacheRun } from './cache-run';
import { getErrorInfo, NodeExecutionError, type RuntimeErrorInfo } from './errors';
import { expandGroups } from './expand-groups';
import { migrateGraph } from './graph-migrations';
import { InstanceTracker } from './instances';
import { buildPlan, type Plan, type PlanNode } from './plan';
import type { DebugEntry } from './types';

export { toCreatorId } from './inputs';

/** The node that turns a value of this socket type into something the debug view can draw */
const DEBUG_PREVIEWS: Record<string, { node: string }> = {
  // a flat disc colored by the pattern
  pattern: { node: 'max/myzel/output' }
};
const log = createLogger('runtime-executor');
log.mute();

type DebugData = Record<number, DebugEntry>;
type Timings = Record<number, number>;

/** Which nodes of a plan run and which are read from the cache */
type Runs = {
  needed: Set<number>;
  cached: Map<number, Int32Array[]>;
};

function cacheEntry(node: PlanNode) {
  return `node-${node.id}-${node.cacheKey}`;
}

/**
 * From the output back: a node runs when the cache doesn't have its result and a node that runs,
 * or a debug node, needs it. So a value the cache doesn't keep is only made again when something
 * below it changed. Every entry of the graph is read, they all belong to this run.
 */
function decideRuns(plan: Plan, cache: CacheRun): Runs {
  const needed = new Set<number>([plan.output.id]);
  const cached = new Map<number, Int32Array[]>();
  for (let i = plan.nodes.length - 1; i >= 0; i--) {
    const node = plan.nodes[i];
    const cachedValue = node.cacheKey !== undefined && node.id !== plan.output.id
      ? cache.read(cacheEntry(node))
      : undefined;
    if (cachedValue !== undefined) cached.set(node.id, cachedValue);
    if (node.kind === 'debug') needed.add(node.id);
    if (!needed.has(node.id)) continue;
    // a debug node shows its inputs, and the amount and clip from them
    if (cachedValue !== undefined && node.kind !== 'debug') continue;
    for (const input of node.inputs) if (!(input instanceof Int32Array)) needed.add(input.node.id);
  }
  return { needed, cached };
}

function storeDebugData(debugData: DebugData, node: PlanNode, values: NodeValue[]) {
  const output = node.debugOutput;
  const outputs = node.definition.outputs;
  if (output === undefined || !outputs) return;
  const value = values[output];
  if (value === undefined) return;
  debugData[node.id] = {
    type: getOutputType(outputs[output]) ?? 'unknown',
    output,
    data: readNodeValue(value)
  };
}

/**
 * The amount and clip of a debug node go along with the data it shows, the viewer applies
 * them. So the output of the debugged node and every cache key stay as they are.
 */
function storeDebugView(debugData: DebugData, node: PlanNode, inputs: NodeValue[]) {
  const keys = Object.keys(node.definition.inputs ?? {});
  const shown = node.inputs[keys.indexOf('input')];
  const entry = shown && !(shown instanceof Int32Array) ? debugData[shown.node.id] : undefined;
  if (!entry) return;
  for (const key of ['amount', 'clip'] as const) {
    const value = inputs[keys.indexOf(key)];
    if (value !== undefined) entry[key] = decodeFloat(readNodeValue(value)[0]);
  }
}

/**
 * Values the viewer can't draw as they are get turned into something it can, by a node of the
 * graph's own collection. A pattern is only numbers, its preview is the mesh of the output node.
 */
function previewDebugData(debugData: DebugData, definitions: Map<string, NodeDefinition>) {
  for (const entry of Object.values(debugData)) {
    const preview = DEBUG_PREVIEWS[entry.type];
    const node = preview && definitions.get(preview.node);
    if (!node) continue;
    try {
      const result = node.execute([entry.data]);
      entry.data = readNodeValue(Array.isArray(result) ? result[0] : result);
    } catch (e) {
      log.warn(`could not preview the ${entry.type} for debugging`, e);
    }
  }
}

/** The nodes of a group run under ids the editor doesn't know, their group node shows the sum */
function addGroupTimings(timings: Timings, origins: Map<number, number>) {
  for (const [id, origin] of origins) {
    if (id in timings) timings[origin] = (timings[origin] ?? 0) + timings[id];
  }
}

export class MemoryRuntimeExecutor implements RuntimeExecutor {
  private instances = new InstanceTracker();

  // keyed by the id of the node a debug node is connected to
  private debugData: DebugData = {};
  // execution time in ms of every node of the last run, cached nodes keep
  // the time of the run that computed their result, group nodes the sum of their nodes
  private nodeTimings: Timings = {};
  // the same, but only of the nodes that ran in the last run and not from the cache
  private runTimings: Timings = {};
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

  /** Instances kept after a run, see {@link KEEP_INSTANCES} */
  get keepInstances() {
    return this.instances.keep;
  }

  set keepInstances(count: number) {
    this.instances.keep = count;
  }

  private async loadDefinitions(graph: Graph) {
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

  async execute(graph: Graph, settings: Record<string, unknown>) {
    this.lastError = undefined;
    const run = this.instances.enterRun();
    try {
      return await this.executeGraph(graph, settings, run);
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

  private async executeGraph(graph: Graph, settings: Record<string, unknown>, run: number) {
    this.perf?.addPoint('runtime');
    const startedAt = performance.now();

    const origins = new Map<number, number>();
    graph = expandGroups(migrateGraph(graph), origins);
    const definitions = await this.loadDefinitions(graph);

    // nothing below this may await
    const plan = buildPlan(graph, definitions, settings, {
      cacheKeys: this.cache !== undefined,
      perf: this.perf,
      startedAt
    });
    this.instances.resetFor(definitions, run);
    const cache = startCacheRun(this.cache, plan.nodes.length);
    const runs = decideRuns(plan, cache);
    const { result, debugData, nodeTimings, runTimings } = this.runPlan(plan, runs, cache, origins);

    // after the result is copied, a preview may run the output node once more
    previewDebugData(debugData, definitions);
    cache.finish();

    addGroupTimings(nodeTimings, origins);
    addGroupTimings(runTimings, origins);
    this.debugData = debugData;
    this.nodeTimings = nodeTimings;
    this.runTimings = runTimings;

    this.perf?.endPoint('runtime');
    return result as unknown as Int32Array;
  }

  private runPlan(plan: Plan, runs: Runs, cache: CacheRun, origins: Map<number, number>) {
    // results of wasm nodes stay inside their module's memory until the next run
    const results: Record<number, NodeValue[]> = {};
    const debugData: DebugData = {};
    const nodeTimings: Timings = {};
    const runTimings: Timings = {};

    for (const node of plan.nodes) {
      const { definition } = node;
      const nodeError = (cause: unknown) =>
        new NodeExecutionError(
          cause,
          { id: origins.get(node.id) ?? node.id, type: node.type },
          definition,
          node.missingInputs
        );

      const needed = runs.needed.has(node.id);
      const cachedValue = runs.cached.get(node.id);
      if (cachedValue !== undefined) results[node.id] = cachedValue;
      if (!needed || cachedValue !== undefined) {
        if (needed) this.perf?.addPoint('cache-hit', 1);
        if (node.id in this.nodeTimings) nodeTimings[node.id] = this.nodeTimings[node.id];
      }
      if (!needed) continue;

      const isDebug = node.kind === 'debug';
      const keys = Object.keys(definition.inputs || {});
      const inputs = cachedValue === undefined || isDebug
        ? node.inputs.map((input, index): NodeValue => {
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

      if (isDebug) storeDebugView(debugData, node, inputs);

      if (cachedValue !== undefined) {
        log.log(`Using cached value for ${definition.id || node.id}`);
        storeDebugData(debugData, node, cachedValue);
        continue;
      }
      this.perf?.addPoint('cache-hit', 0);

      try {
        log.group(`executing ${definition.id}-${node.id}`);
        log.log(`Inputs:`, inputs);
        // a module that has to be instantiated first takes longer than it will the next time
        const cold = definition.memoryBytes?.() === 0;
        const a = performance.now();
        const result = definition.execute(inputs);
        results[node.id] = Array.isArray(result) ? result : [result];
        storeDebugData(debugData, node, results[node.id]);
        log.log('Executed', node.type, node.id);
        const b = performance.now();

        if (node.cacheKey !== undefined && node.id !== plan.output.id) {
          const cost = cold && node.id in this.nodeTimings
            ? Math.min(b - a, this.nodeTimings[node.id])
            : b - a;
          cache.store(cacheEntry(node), results[node.id], cost);
        }

        nodeTimings[node.id] = b - a;
        runTimings[node.id] = b - a;
        this.perf?.addPoint('node/' + definition.id, b - a);
        log.log('Result:', results[node.id]);
        log.groupEnd();
      } catch (e) {
        log.groupEnd();
        throw nodeError(e);
      }
    }

    const outputResult = results[plan.output.id]?.[0];
    const result = outputResult && readNodeValue(outputResult);
    return { result, debugData, nodeTimings, runTimings };
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
