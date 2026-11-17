/** The runs of one benchmark as `benchmark/out/<name>.json` stores them, ms per key and run */
export type RunData = Record<string, number[]>[];

export type NodeTime = { node: string; ms: number };

export type Summary = {
  name: string;
  cached: boolean;
  runs: number;
  /** of the time of the runtime per run, ms */
  median: number;
  p95: number;
  /** median absolute deviation over the median, 0.04 is 4 % */
  spread: number;
  vertices: number;
  /** the median ms of every node type per run, slowest first */
  nodes: NodeTime[];
  cpuModel?: string;
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

const sum = (values: number[] | undefined) => (values ?? []).reduce((a, b) => a + b, 0);

export function summarize(
  name: string,
  data: RunData,
  cached: boolean,
  cpuModel?: string
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
    runs: data.length,
    median: median(totals),
    p95: percentile(totals, 95),
    spread: spread(totals),
    vertices: median(data.map(run => sum(run['total-vertices']))),
    nodes,
    cpuModel
  };
}

export type Verdict = 'faster' | 'slower' | 'within noise';

export type Change = {
  before: number;
  after: number;
  /** after / before - 1, -0.09 is 9 % faster */
  change: number;
  verdict: Verdict;
};

/**
 * A change counts only when it is larger than the spread of both runs, a 3 % change of runs that
 * scatter by 5 % is nothing
 */
export function compareMedians(
  before: { median: number; spread: number },
  after: { median: number; spread: number }
): Change {
  const change = before.median > 0 ? after.median / before.median - 1 : 0;
  const real = Math.abs(change) > before.spread && Math.abs(change) > after.spread;
  return {
    before: before.median,
    after: after.median,
    change,
    verdict: !real ? 'within noise' : change < 0 ? 'faster' : 'slower'
  };
}

/** A slowdown that `--compare` fails on: real, and larger than the threshold (0.1 is 10 %) */
export function isRegression(change: Change, threshold: number) {
  return change.verdict === 'slower' && change.change > threshold;
}
