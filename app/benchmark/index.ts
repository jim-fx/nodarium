// Measures whole graphs through the runtime and writes the runs to benchmark/out/<name>.json,
// which CI pushes to the benchmarks repository. Prints a summary, compares with a saved run.
// `pnpm bench --help` lists the flags, they are in BENCH_USAGE of args.ts.
import type { Graph } from '@nodarium/types';
import { spawn } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import {
  BENCH_USAGE,
  type BenchOptions,
  parseBenchArgs,
  processCount,
  type Runs,
  runsFor,
  UsageError,
  userPath
} from './args.ts';
import { compareSummaries, comparisonTable, summaryTable } from './format.ts';
import { log, run } from './measure.ts';
import {
  BUSY_CPU,
  combineProcesses,
  isRegression,
  ONE_PROCESS_LIMIT,
  type RunData,
  summarize,
  type Summary
} from './stats.ts';
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
  metadata: {
    benchmark: { cached: boolean };
    machine: { cpuModel: string };
    system: { averages: { otherCpuPercent: number } };
  };
};

function summaryOf(name: string, { data, metadata }: ResultFile) {
  return summarize(
    name,
    data,
    metadata.benchmark.cached,
    metadata.machine.cpuModel,
    metadata.system.averages.otherCpuPercent
  );
}

const PROCESS_DIR = /^process-\d+$/;

const keyOf = (s: Summary) => s.name + (s.cached ? '-cached' : '');

const isResultFolder = (dir: string) =>
  readdirSync(dir, { withFileTypes: true }).some(e =>
    (e.isFile() && e.name.endsWith('.json')) || (e.isDirectory() && PROCESS_DIR.test(e.name))
  );

/** A saved name, or a folder of result files, or a folder of such folders (its latest) */
function comparedFolder(compare: string) {
  const saved = join(SAVED, compare);
  if (existsSync(saved)) return saved;
  let dir = userPath(compare);
  if (!existsSync(dir) || !statSync(dir).isDirectory()) {
    throw new UsageError(`--compare ${compare}: there is no saved run and no folder of that name`);
  }
  if (!isResultFolder(dir)) {
    const runs = readdirSync(dir, { withFileTypes: true }).filter(e => e.isDirectory())
      .map(e => e.name);
    if (!runs.length) throw new UsageError(`--compare ${compare}: no result files in it`);
    runs.sort((a, b) => Number(a) - Number(b) || a.localeCompare(b));
    dir = join(dir, runs.at(-1)!);
  }
  return dir;
}

async function readProcess(dir: string): Promise<Summary[]> {
  const files = (await readdir(dir)).filter(f => f.endsWith('.json')).sort();
  return Promise.all(files.map(async f => {
    const file = JSON.parse(await readFile(join(dir, f), 'utf8')) as ResultFile;
    const name = f.slice(0, -'.json'.length);
    return summaryOf(file.metadata.benchmark.cached ? name.replace(/-cached$/, '') : name, file);
  }));
}

/** The result files of one process, or `process-<n>` folders of several */
async function readSummaries(dir: string): Promise<Summary[]> {
  const processes = (await readdir(dir)).filter(f => PROCESS_DIR.test(f)).sort();
  if (!processes.length) return readProcess(dir);
  const all = (await Promise.all(processes.map(p => readProcess(join(dir, p))))).flat();
  const keys = [...new Set(all.map(keyOf))];
  return keys.map(k => combineProcesses(all.filter(s => keyOf(s) === k)));
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

/** Runs the benchmarks in this process, the result files go to `outPath` */
async function measure(
  list: Awaited<ReturnType<typeof benchmarks>>,
  options: BenchOptions,
  outPath: string
) {
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
  return written;
}

/** The same benchmarks in a fresh process of this script, which writes them to `outPath` */
function measureInProcess(options: BenchOptions, outPath: string) {
  const args = [...options.targets, '--repeat', '1', '--out', outPath];
  if (options.runs !== undefined) args.push('--runs', String(options.runs));
  if (options.warmups !== undefined) args.push('--warmups', String(options.warmups));
  if (!options.cached) args.push('--no-cached');
  if (options.quick) args.push('--quick');
  const child = spawn(process.execPath, [...process.execArgv, process.argv[1], ...args], {
    stdio: ['ignore', 'ignore', 'inherit']
  });
  return new Promise<void>((resolve, reject) =>
    child.on(
      'exit',
      code => code === 0 ? resolve() : reject(new Error(`the bench process exited with ${code}`))
    )
  );
}

async function main(options: BenchOptions) {
  const started = performance.now();
  const outPath = options.out ? userPath(options.out) : OUT;
  const compared = options.compare === undefined ? undefined : comparedFolder(options.compare);
  const list = await benchmarks(options);
  const processes = processCount(options);

  let summaries: Summary[];
  if (processes === 1) {
    const written = await measure(list, options, outPath);
    if (options.save) {
      const dir = join(SAVED, options.save);
      await rm(dir, { recursive: true, force: true });
      await mkdir(dir, { recursive: true });
      await Promise.all(written.map(({ file }) => cp(file, join(dir, file.split('/').pop()!))));
      log('saved to ' + dir);
    }
    summaries = written.map(w => summaryOf(w.name, w.result));
  } else {
    const root = options.save ? join(SAVED, options.save) : join(outPath, 'processes');
    await rm(root, { recursive: true, force: true });
    for (let i = 1; i <= processes; i++) {
      log(`process ${i} of ${processes}`);
      await measureInProcess(options, join(root, `process-${i}`));
    }
    // benchmark/out gets the files of the last process, as one run without --repeat writes them,
    // and without --save the files of all of them in out/processes
    await mkdir(outPath, { recursive: true });
    await cp(join(root, `process-${processes}`), outPath, { recursive: true });
    const order = list.flatMap(b => (options.cached ? [b.name, b.name + '-cached'] : [b.name]));
    summaries = (await readSummaries(root))
      .sort((a, b) => order.indexOf(keyOf(a)) - order.indexOf(keyOf(b)));
    if (options.save) log('saved to ' + root);
  }
  log(`took ${Math.round((performance.now() - started) / 1000)} s`);
  const busy = summaries.filter(s => (s.otherCpu ?? 0) > BUSY_CPU);
  if (busy.length) {
    log(
      `warning: other processes used more than ${BUSY_CPU} % of the machine while `
        + busy.map(s => s.name + (s.cached ? '*' : '')).join(', ') + ' ran'
    );
  }

  const note = processes === 1
    ? '± is the spread of the runs of one process, between processes the median moves more'
    : `± is the range of the medians of ${processes} processes`;
  if (!compared) {
    print(
      options.json
        ? JSON.stringify({ processes, summaries }, null, 2)
        : `${summaryTable(summaries)}\n${note}`
    );
    return 0;
  }

  const before = await readSummaries(compared);
  // a first look: its verdicts are "can't tell", it never fails
  const comparisons = compareSummaries(
    before,
    summaries,
    options.quick ? Infinity : ONE_PROCESS_LIMIT
  );
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
    const side = (summary: Summary | undefined) =>
      `${summary?.processes ?? 0} process${summary?.processes === 1 ? '' : 'es'}`;
    print(
      `before: ${compared} (${cpuBefore}, ${side(before[0])})\n`
        + `after:  this run (${cpuAfter}, ${side(summaries[0])})\n`
    );
    print(comparisonTable(comparisons));
    if (options.quick) {
      print(
        "\ncan't tell: --quick is a first look in one process, it doesn't decide whether"
          + ' something changed\nand never fails. Run --compare without --quick for that.'
      );
    } else if (comparisons.some(c => c.total?.verdict === 'one process')) {
      print(
        "\ncan't tell: one process on a side, the scatter between processes is unknown and a"
          + ' change of\na few percent can be noise. Only a change beyond twice or half counts. A'
          + ' folder of CI has one\nprocess per benchmark, --save and --compare run 3 unless'
          + ' --repeat says otherwise.'
      );
    }
    const busy = comparisons.filter(c => c.total?.verdict === 'busy');
    if (busy.length) {
      print(
        `\ncan't tell: other processes used more than ${BUSY_CPU} % of the machine while `
          + busy.map(c => c.name + (c.cached ? '*' : '')).join(', ')
          + ' ran.\nOnly a change beyond twice or half counts, run again on a quiet machine.'
      );
    }
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
