import { isAbsolute, resolve } from 'node:path';
import { parseArgs, type ParseArgsOptionsConfig } from 'node:util';

export const BENCH_USAGE = `Usage: pnpm bench [target...] [options]

  target           a template of benchmark/templates, a template id of the runtime or the path
                   of a graph or project file. Without one: the six templates of
                   benchmark/templates, each with and without the cache, as CI runs them
  --runs n         runs per benchmark, by default 100 for a plant, 8 for a mushroom and 50
                   for a mushroom with the cache
  --warmups n      runs before the measured ones, by default 10, 1 and 5
  --no-cached      only the runs without the cache
  --quick          a tenth of the runs and warmups, at least 3 runs
  --save name      copies the result files to benchmark/saved/<name>/
  --compare x      compares with benchmark/saved/<x>/ or a folder of result files, like
                   data/main/<timestamp> of the benchmarks repository (data/main: the latest)
  --threshold p    --compare exits with 1 when a benchmark got slower by more than p %, 10
  --json           prints the summary as JSON
  --out dir        where the result files go, benchmark/out by default
`;

export const PROFILE_USAGE = `Usage: pnpm run profile <target> [options]

  target               a template of benchmark/templates, a template id of the runtime or the
                       path of a graph or project file
  --seed n             1 by default
  --builds n           builds of the graph, the first is cold, 3 by default
  --cache              runs with the cache of the runtime and the same seed
  --set key=value      a setting (resolution.curve=16) or an input of a node (4.length=0.3)
  --scale t.prop=f     multiplies the input prop of every node of type t by f, like
                       max/plantarium/instance.amount=4 or instance.amount=4
  --json               prints JSON
  --dump-args dir      writes the inputs of every node to dir/<node>.<call>.args,
                       for pnpm profile:node
`;

export const PROFILE_NODE_USAGE = `Usage: pnpm profile:node <node> <args file> [options]

  node          the id of a node (max/myzel/strands) or its name (strands)
  args file     inputs written by pnpm run profile --dump-args
  --runs n      5 by default
  --wasm dir    the compiled nodes, as <dir>/<user>/<collection>/<node>.wasm, static/nodes by
                default
  --cpu-prof    writes a .cpuprofile next to the args file and prints the 10 functions with
                the most time of their own
`;

export class UsageError extends Error {}

function count(text: string | undefined, flag: string, min = 1) {
  if (text === undefined) return undefined;
  const value = Number(text);
  if (!Number.isInteger(value) || value < min) {
    throw new UsageError(`--${flag} takes a whole number from ${min}, not ${text}`);
  }
  return value;
}

function parse<T extends ParseArgsOptionsConfig>(args: string[], options: T) {
  try {
    return parseArgs({ args, options, allowPositionals: true, strict: true });
  } catch (error) {
    throw new UsageError((error as Error).message);
  }
}

/** Paths are typed relative to where pnpm was started, `pnpm -C app` runs in app/ */
export function userPath(path: string, cwd = process.env.INIT_CWD ?? process.cwd()) {
  return isAbsolute(path) ? path : resolve(cwd, path);
}

export type BenchOptions = {
  targets: string[];
  runs?: number;
  warmups?: number;
  cached: boolean;
  quick: boolean;
  save?: string;
  compare?: string;
  /** 0.1 is 10 % */
  threshold: number;
  json: boolean;
  out?: string;
  help: boolean;
};

export function parseBenchArgs(args: string[]): BenchOptions {
  const { values, positionals } = parse(args, {
    runs: { type: 'string' },
    warmups: { type: 'string' },
    'no-cached': { type: 'boolean' },
    quick: { type: 'boolean' },
    save: { type: 'string' },
    compare: { type: 'string' },
    threshold: { type: 'string', default: '10' },
    json: { type: 'boolean' },
    out: { type: 'string' },
    help: { type: 'boolean', short: 'h' }
  });
  const threshold = Number(values.threshold);
  if (!(threshold >= 0)) {
    throw new UsageError(`--threshold takes a percentage, not ${values.threshold}`);
  }
  if (values.save !== undefined && !/^[\w.-]+$/.test(values.save)) {
    throw new UsageError(`--save takes a name of letters, digits, ".", "_" and "-"`);
  }
  return {
    targets: positionals,
    runs: count(values.runs, 'runs'),
    warmups: count(values.warmups, 'warmups', 0),
    cached: !values['no-cached'],
    quick: values.quick ?? false,
    save: values.save,
    compare: values.compare,
    threshold: threshold / 100,
    json: values.json ?? false,
    out: values.out,
    help: values.help ?? false
  };
}

export type Runs = { runs: number; warmups: number };

/** `--runs` and `--warmups` win over the defaults, `--quick` only shortens the defaults */
export function runsFor(defaults: Runs, options: Pick<BenchOptions, 'runs' | 'warmups' | 'quick'>) {
  const quick = options.quick
    ? {
      runs: Math.max(3, Math.ceil(defaults.runs / 10)),
      warmups: Math.ceil(defaults.warmups / 10)
    }
    : defaults;
  return { runs: options.runs ?? quick.runs, warmups: options.warmups ?? quick.warmups };
}

export type Scale = { type: string; prop: string; factor: number };

export type ProfileOptions = {
  target?: string;
  seed: number;
  builds: number;
  cache: boolean;
  set: string[];
  scale: Scale[];
  json: boolean;
  dumpArgs?: string;
  help: boolean;
};

/** `max/plantarium/instance.amount=4`, the type may be the name of a node alone */
export function parseScale(text: string): Scale {
  const match = text.match(/^(.+)\.([^.=]+)=(.+)$/);
  const factor = Number(match?.[3]);
  if (!match || !Number.isFinite(factor)) {
    throw new UsageError(`--scale ${text}: write type.prop=factor, like instance.amount=4`);
  }
  return { type: match[1], prop: match[2], factor };
}

export function parseProfileArgs(args: string[]): ProfileOptions {
  const { values, positionals } = parse(args, {
    seed: { type: 'string', default: '1' },
    builds: { type: 'string', default: '3' },
    cache: { type: 'boolean' },
    set: { type: 'string', multiple: true },
    scale: { type: 'string', multiple: true },
    json: { type: 'boolean' },
    'dump-args': { type: 'string' },
    help: { type: 'boolean', short: 'h' }
  });
  if (positionals.length > 1) {
    throw new UsageError(`profile takes one target, not ${positionals.join(', ')}`);
  }
  return {
    target: positionals[0],
    seed: count(values.seed, 'seed', 0)!,
    builds: count(values.builds, 'builds')!,
    cache: values.cache ?? false,
    set: values.set ?? [],
    scale: (values.scale ?? []).map(parseScale),
    json: values.json ?? false,
    dumpArgs: values['dump-args'],
    help: values.help ?? false
  };
}

export type ProfileNodeOptions = {
  node?: string;
  args?: string;
  runs: number;
  wasm?: string;
  cpuProf: boolean;
  help: boolean;
};

export function parseProfileNodeArgs(args: string[]): ProfileNodeOptions {
  const { values, positionals } = parse(args, {
    runs: { type: 'string', default: '5' },
    wasm: { type: 'string' },
    'cpu-prof': { type: 'boolean' },
    help: { type: 'boolean', short: 'h' }
  });
  if (positionals.length > 2) {
    throw new UsageError(
      `profile:node takes a node and an args file, not ${positionals.join(', ')}`
    );
  }
  return {
    node: positionals[0],
    args: positionals[1],
    runs: count(values.runs, 'runs')!,
    wasm: values.wasm,
    cpuProf: values['cpu-prof'] ?? false,
    help: values.help ?? false
  };
}
