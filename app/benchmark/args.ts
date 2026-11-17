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
