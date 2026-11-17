// Measures whole graphs through the runtime and writes the runs to benchmark/out/<name>.json,
// which CI pushes to the benchmarks repository. Prints a summary, compares with a saved run.
// `pnpm bench --help` lists the flags, they are in BENCH_USAGE of args.ts.
import type { Graph } from '@nodarium/types';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import {
  BENCH_USAGE,
  type BenchOptions,
  parseBenchArgs,
  type Runs,
  runsFor,
  UsageError,
  userPath
} from './args.ts';
import { compareSummaries, comparisonTable, summaryTable } from './format.ts';
import { log, run } from './measure.ts';
import { isRegression, type RunData, summarize, type Summary } from './stats.ts';
import { isMushroom, loadTarget } from './targets.ts';

type Benchmark = Runs & { cachedRuns?: Runs };

const plantRuns = { runs: 100, warmups: 10 };
// a mushroom build takes seconds, fewer runs keep the bench within a few minutes on CI. A cached
// run takes milliseconds, with only a few of them one pause moves the median
const mushroomRuns = { runs: 8, warmups: 1, cachedRuns: { runs: 50, warmups: 5 } };

/** What runs without a target, in this order */
const DEFAULT_TEMPLATES: Record<string, Benchmark> = {
  plant: plantRuns,
  'lotta-faces': plantRuns,
  default: plantRuns,
  'champignon-groomed': mushroomRuns,
  'fly-agaric': mushroomRuns,
  porcini: mushroomRuns
};

const SAVED = join(import.meta.dirname, 'saved');
const OUT = join(import.meta.dirname, 'out');

const print = (text: string) => process.stdout.write(text + '\n');

type ResultFile = {
  data: RunData;
  metadata: { benchmark: { cached: boolean }; machine: { cpuModel: string } };
};

function summaryOf(name: string, file: ResultFile) {
  return summarize(name, file.data, file.metadata.benchmark.cached, file.metadata.machine.cpuModel);
}

/** A saved name, or a folder of result files, or a folder of such folders (its latest) */
function comparedFolder(compare: string) {
  const saved = join(SAVED, compare);
  if (existsSync(saved)) return saved;
  let dir = userPath(compare);
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    throw new UsageError(`--compare ${compare}: there is no saved run and no folder of that name`);
  }
  const entries = readdirSync(dir, { withFileTypes: true });
  if (!entries.some(e => e.isFile() && e.name.endsWith('.json'))) {
    const runs = entries.filter(e => e.isDirectory()).map(e => e.name);
    if (!runs.length) throw new UsageError(`--compare ${compare}: no result files in it`);
    runs.sort((a, b) => Number(a) - Number(b) || a.localeCompare(b));
    dir = join(dir, runs.at(-1)!);
  }
  return dir;
}

async function readSummaries(dir: string): Promise<Summary[]> {
  const files = (await readdir(dir)).filter(f => f.endsWith('.json')).sort();
  return Promise.all(files.map(async f => {
    const file = JSON.parse(await readFile(join(dir, f), 'utf8')) as ResultFile;
    const name = f.slice(0, -'.json'.length);
    return summaryOf(file.metadata.benchmark.cached ? name.replace(/-cached$/, '') : name, file);
  }));
}

async function benchmarks(options: BenchOptions) {
  if (!options.targets.length) {
    return Promise.all(
      Object.entries(DEFAULT_TEMPLATES).map(async ([name, runs]) => ({
        ...(await loadTarget(name)),
        ...runs
      }))
    );
  }
  return Promise.all(options.targets.map(async target => {
    const loaded = await loadTarget(target);
    return { ...loaded, ...(DEFAULT_TEMPLATES[target] ?? defaultsFor(loaded.graph)) };
  }));
}

function defaultsFor(graph: Graph): Benchmark {
  return isMushroom(graph) ? mushroomRuns : plantRuns;
}

async function main(options: BenchOptions) {
  const outPath = options.out ? userPath(options.out) : OUT;
  const compared = options.compare === undefined ? undefined : comparedFolder(options.compare);
  const list = await benchmarks(options);

  await mkdir(outPath, { recursive: true });

  const written: { file: string; name: string; result: ResultFile }[] = [];
  for (const { name: key, graph, cachedRuns, ...defaults } of list) {
    for (const cached of options.cached ? [false, true] : [false]) {
      const name = cached ? key + '-cached' : key;
      log('executing ' + name);

      const runs = runsFor(cached && cachedRuns ? cachedRuns : defaults, options);
      const perfData = await run({ graph, ...runs }, cached);

      const file = join(outPath, name + '.json');
      await writeFile(file, JSON.stringify(perfData));
      written.push({ file, name: key, result: perfData });

      await new Promise(res => setTimeout(res, 200));
    }
  }

  if (options.save) {
    const dir = join(SAVED, options.save);
    await rm(dir, { recursive: true, force: true });
    await mkdir(dir, { recursive: true });
    await Promise.all(written.map(({ file }) => cp(file, join(dir, file.split('/').pop()!))));
    log('saved to ' + dir);
  }

  const summaries = written.map(w => summaryOf(w.name, w.result));
  if (!compared) {
    print(options.json ? JSON.stringify({ summaries }, null, 2) : summaryTable(summaries));
    return 0;
  }

  const before = await readSummaries(compared);
  const comparisons = compareSummaries(before, summaries);
  const regressions = comparisons.filter(c => c.total && isRegression(c.total, options.threshold));
  const cpuBefore = before[0]?.cpuModel;
  const cpuAfter = summaries[0]?.cpuModel;
  if (cpuBefore !== cpuAfter) {
    log(`warning: the compared run was measured on another CPU, the numbers differ for that alone`);
  }
  if (options.json) {
    print(JSON.stringify(
      { compared, cpu: { before: cpuBefore, after: cpuAfter }, comparisons, summaries },
      null,
      2
    ));
  } else {
    print(`before: ${compared} (${cpuBefore})\nafter:  this run (${cpuAfter})\n`);
    print(comparisonTable(comparisons));
    if (regressions.length) {
      print(
        `\nslower by more than ${Math.round(options.threshold * 100)} %: `
          + regressions.map(c => c.name + (c.cached ? '*' : '')).join(', ')
      );
    }
  }
  return regressions.length ? 1 : 0;
}

let options: BenchOptions;
try {
  options = parseBenchArgs(process.argv.slice(2));
} catch (error) {
  if (!(error instanceof UsageError)) throw error;
  console.error(`${error.message}\n\n${BENCH_USAGE}`);
  process.exit(1);
}
if (options.help) {
  print(BENCH_USAGE);
  process.exit(0);
}

// the runtime and the nodes log through console.log, stdout is for the summary
console.log = console.info = console.error;

try {
  process.exitCode = await main(options);
} catch (error) {
  if (!(error instanceof UsageError)) throw error;
  console.error(error.message);
  process.exitCode = 1;
}
