import type {
  Graph,
  NodeDefinition,
  NodeRegistry,
  NodeValue,
  RuntimeExecutor,
  SyncCache
} from '@nodarium/types';
import { INTERNAL_PREFIX } from '@nodarium/types';
import { createLogger, type PerformanceStore, readNodeValue } from '@nodarium/utils';
import { type CacheRun, startCacheRun } from './cache-run';
import { DebugCollector } from './debug-data';
import { getErrorInfo, NodeExecutionError, type RuntimeErrorInfo } from './errors';
import { expandGroups } from './expand-groups';
import { migrateGraph } from './graph-migrations';
import { InstanceTracker } from './instances';
import { buildPlan, type Plan, type PlanNode } from './plan';
import type { RunResult } from './types';

export { toCreatorId } from './inputs';

const log = createLogger('runtime-executor');
log.mute();

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

/** The nodes of a group run under ids the editor doesn't know, their group node shows the sum */
function addGroupTimings(timings: Timings, origins: Map<number, number>) {
  for (const [id, origin] of origins) {
    if (id in timings) timings[origin] = (timings[origin] ?? 0) + timings[id];
  }
}

export class MemoryRuntimeExecutor implements RuntimeExecutor {
  private instances = new InstanceTracker();

  // what the getters read, of the last run that succeeded. Its result belongs to the caller
  private last?: Omit<RunResult, 'result'>;
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
    return (await this.run(graph, settings)).result;
  }

  /** Like {@link execute}, with the debug data and the timings of this run */
  async run(graph: Graph, settings: Record<string, unknown>): Promise<RunResult> {
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

  private async executeGraph(
    graph: Graph,
    settings: Record<string, unknown>,
    run: number
  ): Promise<RunResult> {
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
    const { result, debug, nodeTimings, runTimings } = this.runPlan(plan, runs, cache, origins);

    // after the result is copied, a preview may run the output node once more
    debug.preview(definitions);
    cache.finish();

    addGroupTimings(nodeTimings, origins);
    addGroupTimings(runTimings, origins);
    this.last = { debugData: debug.data, nodeTimings, runTimings };

    this.perf?.endPoint('runtime');
    return { result: result as unknown as Int32Array, ...this.last };
  }

  private runPlan(plan: Plan, runs: Runs, cache: CacheRun, origins: Map<number, number>) {
    // results of wasm nodes stay inside their module's memory until the next run
    const results: Record<number, NodeValue[]> = {};
    const debug = new DebugCollector();
    const nodeTimings: Timings = {};
    const runTimings: Timings = {};
    const previousTimings = this.last?.nodeTimings ?? {};

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
        if (node.id in previousTimings) nodeTimings[node.id] = previousTimings[node.id];
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

      if (isDebug) debug.applyView(node, inputs);

      if (cachedValue !== undefined) {
        log.log(`Using cached value for ${definition.id || node.id}`);
        debug.record(node, cachedValue);
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
        // counts into the time of the node, by which the cache decides whether it keeps the result
        debug.record(node, results[node.id]);
        log.log('Executed', node.type, node.id);
        const b = performance.now();

        if (node.cacheKey !== undefined && node.id !== plan.output.id) {
          const cost = cold && node.id in previousTimings
            ? Math.min(b - a, previousTimings[node.id])
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
    return { result, debug, nodeTimings, runTimings };
  }

  getDebugData() {
    return this.last?.debugData ?? {};
  }

  getPerformanceData() {
    return this.perf?.get();
  }

  getNodeTimings() {
    return this.last?.nodeTimings ?? {};
  }

  getRunTimings() {
    return this.last?.runTimings ?? {};
  }

  /** Details of the error of the last run, if it failed */
  getLastError() {
    return this.lastError;
  }
}
