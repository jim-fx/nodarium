/** The runs of one benchmark as `benchmark/out/<name>.json` stores them, ms per key and run */
export type RunData = Record<string, number[]>[];

export type NodeTime = { node: string; ms: number };

export type Summary = {
  name: string;
  cached: boolean;
  /** the fresh processes the benchmark ran in */
  processes: number;
  runs: number;
  /** of the time of the runtime per run, ms; of several processes the median of theirs */
  median: number;
  p95: number;
  /**
   * 0.04 is 4 %. Of one process the median absolute deviation of its runs over the median, of
   * several the range of their medians over the median: the median moves more between two
   * processes than between two runs of one
   */
  spread: number;
  vertices: number;
  /** the median ms of every node type per run, slowest first */
  nodes: NodeTime[];
  cpuModel?: string;
  /** percent of the machine other processes used while it ran, of several the median */
  otherCpu?: number;
};

export function median(values: number[]) {
  if (values.length === 0) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

/** Nearest rank: the smallest value that `p` percent of the values are at or below */
export function percentile(values: number[], p: number) {
  if (values.length === 0) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

/** The median absolute deviation over the median, robust against the one run a pause hit */
export function spread(values: number[]) {
  const m = median(values);
  if (!(m > 0)) return 0;
  return median(values.map(v => Math.abs(v - m))) / m;
}

/** The range of the values over their median */
export function range(values: number[]) {
  const m = median(values);
  if (!(m > 0)) return 0;
  return (Math.max(...values) - Math.min(...values)) / m;
}

const sum = (values: number[] | undefined) => (values ?? []).reduce((a, b) => a + b, 0);

export function summarize(
  name: string,
  data: RunData,
  cached: boolean,
  cpuModel?: string,
  otherCpu?: number
): Summary {
  const totals = data.map(run => sum(run.total));
  const types = new Set(data.flatMap(run => Object.keys(run).filter(k => k.startsWith('node/'))));
  // a node that didn't run in a run, like one served from the cache, took 0 ms in it
  const nodes = [...types]
    .map(key => ({ node: key.slice('node/'.length), ms: median(data.map(run => sum(run[key]))) }))
    .sort((a, b) => b.ms - a.ms);
  return {
    name,
    cached,
    processes: 1,
    runs: data.length,
    median: median(totals),
    p95: percentile(totals, 95),
    spread: spread(totals),
    vertices: median(data.map(run => sum(run['total-vertices']))),
    nodes,
    cpuModel,
    otherCpu
  };
}

/** One benchmark run in several processes: the medians of theirs, the range as the spread */
export function combineProcesses(summaries: Summary[]): Summary {
  if (summaries.length === 1) return summaries[0];
  const types = [...new Set(summaries.flatMap(s => s.nodes.map(n => n.node)))];
  const of = (pick: (s: Summary) => number) => median(summaries.map(pick));
  return {
    name: summaries[0].name,
    cached: summaries[0].cached,
    processes: sum(summaries.map(s => s.processes)),
    runs: sum(summaries.map(s => s.runs)),
    median: of(s => s.median),
    p95: of(s => s.p95),
    spread: range(summaries.map(s => s.median)),
    vertices: of(s => s.vertices),
    nodes: types
      .map(node => ({ node, ms: of(s => s.nodes.find(n => n.node === node)?.ms ?? 0) }))
      .sort((a, b) => b.ms - a.ms),
    cpuModel: summaries[0].cpuModel,
    otherCpu: summaries.some(s => s.otherCpu !== undefined)
      ? of(s => s.otherCpu ?? 0)
      : undefined
  };
}

/**
 * How far apart the medians of two sides have to be, in their spreads added. Simulated with 3
 * processes per side and normal or log-normal scatter, 1 calls about 1.5 % of the comparisons of
 * unchanged code a change, 2 about 0.15 %: with 12 benchmarks 1 in 50 runs of `--compare`
 * instead of 1 in 6. A 10 % change of a mushroom, whose median scatters by about 1 % between
 * processes, still counts.
 */
export const NOISE_FACTOR = 2;

/**
 * With one process on a side the scatter between processes is unknown. Measured on one machine,
 * the median of a benchmark moved by up to 85 % between processes (`plant*`, 0.059 to 0.109 ms),
 * so only a change beyond twice or half counts.
 */
export const ONE_PROCESS_LIMIT = 1;

/**
 * Percent of the machine other processes may use while a benchmark runs. Next to other work
 * (a browser, builds and tests of other worktrees) they used 5 to 55 % of the 6 cores of the
 * machine this was measured on, and the medians of unchanged code stayed within their spread
 * between processes all the same: a load that changes between processes widens that spread.
 * Beyond this a side's median isn't trusted, then only a change beyond
 * {@link ONE_PROCESS_LIMIT} counts.
 */
export const BUSY_CPU = 60;

/** `one process` and `busy`: can't tell noise from a change, see {@link compareMedians} */
export type Verdict = 'faster' | 'slower' | 'within noise' | 'one process' | 'busy';

export type Change = {
  before: number;
  after: number;
  /** after / before - 1, -0.09 is 9 % faster */
  change: number;
  verdict: Verdict;
};

type Side = Pick<Summary, 'median' | 'spread' | 'processes' | 'otherCpu'>;

/**
 * A change counts when the medians lie further apart than {@link NOISE_FACTOR} times the
 * spreads between the processes of both sides added. With one process on a side that spread
 * isn't known, and on a busy machine ({@link BUSY_CPU}) it isn't the one of a quiet one: then
 * only a change beyond `oneProcessLimit` counts, {@link ONE_PROCESS_LIMIT} by default.
 */
export function compareMedians(
  before: Side,
  after: Side,
  oneProcessLimit = ONE_PROCESS_LIMIT
): Change {
  const change = before.median > 0 ? after.median / before.median - 1 : 0;
  const result = { before: before.median, after: after.median, change };
  const unknown: Verdict | undefined = Math.min(before.processes, after.processes) < 2
    ? 'one process'
    : Math.max(before.otherCpu ?? 0, after.otherCpu ?? 0) > BUSY_CPU
    ? 'busy'
    : undefined;
  const real = unknown
    ? change > oneProcessLimit || change < 1 / (1 + oneProcessLimit) - 1
    : Math.abs(change) > NOISE_FACTOR * (before.spread + after.spread);
  const verdict = real ? (change < 0 ? 'faster' : 'slower') : unknown ?? 'within noise';
  return { ...result, verdict };
}

/** A slowdown that `--compare` fails on: real, and larger than the threshold (0.1 is 10 %) */
export function isRegression(change: Change, threshold: number) {
  return change.verdict === 'slower' && change.change > threshold;
}
