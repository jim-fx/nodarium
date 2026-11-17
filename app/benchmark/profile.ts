// The time and the memory of every node of one graph, through the runtime as the app runs it.
// `pnpm run profile --help` lists the flags, they are in PROFILE_USAGE of args.ts.
import {
  expandGroups,
  FileNodeRegistry,
  MemoryRuntimeCache,
  MemoryRuntimeExecutor,
  migrateGraph,
  parseSets
} from '@nodarium/runtime/node';
import type { Graph, NodeId } from '@nodarium/types';
import { createPerformanceStore } from '@nodarium/utils';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  parseProfileArgs,
  PROFILE_USAGE,
  type ProfileOptions,
  UsageError,
  userPath
} from './args.ts';
import { bytes, ms, percent, table } from './format.ts';
import { argsFileName, encodeArgs } from './node-args.ts';
import { ProfilingRegistry } from './profiling-registry.ts';
import { median } from './stats.ts';
import { loadTarget, NODES_DIR, scaleProps } from './targets.ts';

type NodeRow = {
  type: string;
  runs: number;
  ms: number;
  /** inJs + inWasm + inShared */
  inBytes: number;
  inJs: number;
  inWasm: number;
  inShared: number;
  outBytes: number;
  /** the node's wasm memory, the largest after any of its calls */
  wasmBytes: number;
};

type Build = {
  ms: number;
  /** the sum of the nodes */
  nodeMs: number;
  /** what the runtime spends besides the nodes, and the parts of it it measures */
  overhead: { ms: number; metadata: number; inputs: number; hash: number; reset: number };
  outputBytes: number;
  wasmBytes: number;
  rss: number;
  peakRss: number;
  nodes: NodeRow[];
  /** every node that ran, like `nodarium run --timings` lists them */
  calls: { id: number; type: string; ms: number; bytes: number }[];
};

const print = (text: string) => process.stdout.write(text + '\n');
const sum = (values: number[] | undefined) => (values ?? []).reduce((a, b) => a + b, 0);

/** The defaults of the inputs `--scale` changes, for nodes that don't set them */
async function definitionDefaults(graph: Graph) {
  const registry = new FileNodeRegistry(NODES_DIR);
  const types = [...graph.nodes, ...(graph.groups ?? []).flatMap(g => g.nodes)].map(n => n.type);
  await registry.load(types as NodeId[]);
  return (type: string, prop: string) => registry.getNode(type)?.inputs?.[prop]?.value;
}

function nodeRows(
  calls: Build['calls'],
  registry: ProfilingRegistry
): NodeRow[] {
  const rows = new Map<string, NodeRow>();
  const row = (type: string) => {
    let found = rows.get(type);
    if (!found) {
      found = {
        type,
        runs: 0,
        ms: 0,
        inBytes: 0,
        inJs: 0,
        inWasm: 0,
        inShared: 0,
        outBytes: 0,
        wasmBytes: 0
      };
      rows.set(type, found);
    }
    return found;
  };
  for (const call of calls) {
    const r = row(call.type);
    r.runs++;
    r.ms += call.ms;
    r.outBytes += call.bytes;
  }
  for (const call of registry.calls) {
    const r = row(call.type);
    r.inJs += call.inJs;
    r.inWasm += call.inWasm;
    r.inShared += call.inShared;
    r.inBytes += call.inJs + call.inWasm + call.inShared;
    r.wasmBytes = Math.max(r.wasmBytes, call.memory);
  }
  return [...rows.values()].sort((a, b) => b.ms - a.ms);
}

async function profile(options: ProfileOptions) {
  const sets = parseSets(options.set);
  const target = await loadTarget(options.target!, sets.props);
  if (options.scale.length) {
    const fallback = await definitionDefaults(target.graph);
    const changed = scaleProps(target.graph, options.scale, fallback);
    options.scale.forEach((s, i) => {
      if (!changed[i]) {
        throw new UsageError(`--scale: no node of type ${s.type} has a number input ${s.prop}`);
      }
    });
  }

  // expanded here, so the ids of the timings are the ids of the graph's nodes
  const graph = expandGroups(migrateGraph(target.graph), new Map());
  const types = new Map(graph.nodes.map(node => [node.id, node.type]));
  const settings = { ...graph.settings, ...sets.settings, seed: options.seed };

  const registry = new ProfilingRegistry(NODES_DIR);
  const runtime = new MemoryRuntimeExecutor(
    registry,
    options.cache ? new MemoryRuntimeCache() : undefined
  );
  const perf = createPerformanceStore();
  runtime.perf = perf;

  const builds: Build[] = [];
  for (let i = 0; i < options.builds; i++) {
    registry.startBuild();
    perf.startRun();
    const a = performance.now();
    const ran = await runtime.run(graph, settings);
    const total = performance.now() - a;
    perf.stopRun();
    const points = perf.get().at(-1)!;

    const calls = Object.entries(ran.runTimings)
      .filter(([id]) => types.has(Number(id)))
      .map(([id, ms]) => ({
        id: Number(id),
        type: types.get(Number(id))!,
        ms,
        bytes: ran.outputBytes?.[Number(id)] ?? 0
      }));
    const nodes = nodeRows(calls, registry);
    const nodeMs = sum(calls.map(c => c.ms));
    builds.push({
      ms: total,
      nodeMs,
      overhead: {
        ms: total - nodeMs,
        metadata: sum(points['collect-metadata']),
        inputs: sum(points['collected-inputs']),
        hash: sum(points['hash-inputs']),
        reset: registry.resetMs
      },
      outputBytes: ran.result.byteLength,
      wasmBytes: sum(nodes.map(n => n.wasmBytes)),
      rss: process.memoryUsage.rss(),
      peakRss: process.resourceUsage().maxRSS * 1024,
      nodes,
      calls
    });
  }
  runtime.releaseInstances();

  const dumped = options.dumpArgs ? await dumpArgs(options.dumpArgs, graph, settings) : undefined;
  return { name: target.name, settings, builds, dumped };
}

/**
 * One more build on fresh instances that writes the inputs of every node, after the measured
 * builds so the copies don't count into them
 */
async function dumpArgs(dir: string, graph: Graph, settings: Record<string, unknown>) {
  const out = userPath(dir);
  mkdirSync(out, { recursive: true });
  const calls = new Map<string, number>();
  const registry = new ProfilingRegistry(NODES_DIR, (type, inputs) => {
    const call = (calls.get(type) ?? 0) + 1;
    calls.set(type, call);
    writeFileSync(join(out, argsFileName(type, call)), encodeArgs(inputs));
  });
  const runtime = new MemoryRuntimeExecutor(registry);
  await runtime.run(graph, settings);
  runtime.releaseInstances();
  return { dir: out, files: [...calls.values()].reduce((a, b) => a + b, 0) };
}

/** The builds after the first, or the first when it is the only one */
function warmOf(builds: Build[]) {
  const warm = builds.length > 1 ? builds.slice(1) : builds;
  const of = (pick: (b: Build) => number) => median(warm.map(pick));
  const last = warm.at(-1)!;
  const nodes = last.nodes.map(node => {
    const inBuild = (b: Build) => b.nodes.find(n => n.type === node.type);
    return {
      ...node,
      runs: median(warm.map(b => inBuild(b)?.runs ?? 0)),
      ms: median(warm.map(b => inBuild(b)?.ms ?? 0))
    };
  }).sort((a, b) => b.ms - a.ms);
  return {
    label: builds.length > 1 ? 'warm' : 'cold',
    ms: of(b => b.ms),
    overhead: {
      ms: of(b => b.overhead.ms),
      metadata: of(b => b.overhead.metadata),
      inputs: of(b => b.overhead.inputs),
      hash: of(b => b.overhead.hash),
      reset: of(b => b.overhead.reset)
    },
    nodes
  };
}

function report(
  options: ProfileOptions,
  { name, builds, dumped }: Awaited<ReturnType<typeof profile>>
) {
  const warm = warmOf(builds);
  const cold = builds[0];
  const last = builds.at(-1)!;
  const head = [
    name,
    `seed ${options.seed}`,
    ...(options.cache ? ['cached'] : []),
    `cold ${ms(cold.ms)} ms`,
    ...(builds.length > 1 ? [`warm ${ms(warm.ms)} ms`] : []),
    `output ${bytes(last.outputBytes)}`,
    `wasm ${bytes(last.wasmBytes)}`,
    `peak RSS ${bytes(last.peakRss)}`
  ];
  const { overhead } = warm;
  const rows = warm.nodes.map(n => [
    n.type.split('/').pop()!,
    String(n.runs),
    ms(n.ms),
    percent(n.ms / warm.ms),
    bytes(n.inBytes),
    bytes(n.outBytes),
    bytes(n.wasmBytes)
  ]);
  print(head.join('  '));
  print(
    `runtime overhead (${warm.label})  ${ms(overhead.ms)} ms: metadata ${
      ms(overhead.metadata)
    }, inputs ${ms(overhead.inputs)}, hash ${ms(overhead.hash)}, reset ${ms(overhead.reset)}`
  );
  print('');
  print(table([['node', 'runs', 'ms', '%', 'in', 'out', 'wasm'], ...rows], [1, 2, 3, 4, 5, 6]));
  if (dumped) console.error(`wrote ${dumped.files} args files to ${dumped.dir}`);
}

let options: ProfileOptions;
try {
  options = parseProfileArgs(process.argv.slice(2));
  if (!options.help && !options.target) throw new UsageError('profile takes a target');
} catch (error) {
  if (!(error instanceof UsageError)) throw error;
  console.error(`${error.message}\n\n${PROFILE_USAGE}`);
  process.exit(1);
}
if (options.help) {
  print(PROFILE_USAGE);
  process.exit(0);
}

// the runtime and the nodes log through console.log, stdout is for the result
console.log = console.info = console.error;

try {
  const result = await profile(options);
  if (options.json) {
    print(
      JSON.stringify(
        { ...result, seed: options.seed, cache: options.cache, warm: warmOf(result.builds) },
        null,
        2
      )
    );
  } else {
    report(options, result);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
