import { describe, expect, it } from 'vitest';
import { compareSummaries } from './format.ts';
import {
  combineProcesses,
  compareMedians,
  isRegression,
  median,
  percentile,
  range,
  type RunData,
  spread,
  summarize
} from './stats.ts';

describe('median, p95 and spread', () => {
  it('takes the middle value, or the mean of the two middle ones', () => {
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 3, 2])).toBe(2.5);
    expect(median([])).toBeNaN();
  });

  it('takes the p95 by nearest rank', () => {
    const values = Array.from({ length: 100 }, (_, i) => i + 1);
    expect(percentile(values, 95)).toBe(95);
    expect(percentile([5, 1, 9, 3, 7, 2, 8, 4], 95)).toBe(9);
    expect(percentile([7], 95)).toBe(7);
  });

  it('measures the spread as the median absolute deviation over the median', () => {
    // median 10, deviations 0 0 1 1 10, their median 1
    expect(spread([10, 9, 11, 10, 20])).toBeCloseTo(0.1);
    expect(spread([5, 5, 5])).toBe(0);
    expect(spread([0, 0])).toBe(0);
  });

  it('is not moved by one run that a pause hit', () => {
    const runs = [100, 101, 99, 100, 102, 98, 100, 900];
    expect(median(runs)).toBe(100);
    expect(spread(runs)).toBeCloseTo(0.01);
  });
});

describe('summarize', () => {
  const data: RunData = [
    {
      total: [10],
      'node/max/plantarium/stem': [2, 1],
      'node/max/plantarium/noise': [4],
      'total-vertices': [100]
    },
    {
      total: [12],
      'node/max/plantarium/stem': [3, 1],
      'node/max/plantarium/noise': [5],
      'total-vertices': [100]
    },
    { total: [11], 'node/max/plantarium/noise': [6], 'total-vertices': [100] }
  ];

  it('sums the calls of a node per run and sorts the nodes by their median', () => {
    const summary = summarize('plant', data, false);
    expect(summary).toMatchObject({ runs: 3, median: 11, p95: 12, vertices: 100 });
    expect(summary.nodes).toEqual([
      { node: 'max/plantarium/noise', ms: 5 },
      // a run without the node counts as 0 ms
      { node: 'max/plantarium/stem', ms: 3 }
    ]);
  });
});

describe('several processes', () => {
  it('takes the median of their medians and the range of them as the spread', () => {
    const process = (ms: number[]) => summarize('porcini', ms.map(m => ({ total: [m] })), false);
    const combined = combineProcesses([
      process([1430, 1434, 1440]),
      process([1460, 1461, 1470]),
      process([1420, 1425, 1430])
    ]);
    expect(combined).toMatchObject({ processes: 3, runs: 9, median: 1434 });
    expect(combined.spread).toBeCloseTo(range([1434, 1461, 1425]));
    expect(combined.spread).toBeCloseTo(36 / 1434);
  });

  it('takes the median of the load of other processes, one busy process widens the range', () => {
    const process = (otherCpu: number) => summarize('plant', [{ total: [1] }], false, '', otherCpu);
    expect(combineProcesses([process(80), process(12), process(20)]).otherCpu).toBe(20);
  });
});

/** A side of a comparison as several processes measured it, from the medians of theirs */
const side = (medians: number[]) => ({
  median: median(medians),
  spread: range(medians),
  processes: medians.length
});

describe('compare', () => {
  it('calls unchanged code unchanged, as it scattered between processes', () => {
    // medians of unchanged code measured in different processes on one machine
    const porcini = compareMedians(side([1434, 1461, 1425]), side([1457, 1448, 1462]));
    const plantCached = compareMedians(side([0.084, 0.109, 0.059]), side([0.059, 0.067, 0.102]));
    const flyAgaricCached = compareMedians(side([8.1, 7.0, 7.9]), side([8.0, 7.9, 8.0]));
    for (const unchanged of [porcini, plantCached, flyAgaricCached]) {
      expect(unchanged.verdict).toBe('within noise');
    }
  });

  it('tells a slowdown by hand from a second run of unchanged code', () => {
    // porcini, the medians of 3 processes each: saved, unchanged again, every node 20 % slower
    const saved = side([1443.4, 1444.4, 1442.2]);
    expect(compareMedians(saved, side([1447.6, 1470.8, 1471.8])).verdict).toBe('within noise');
    expect(compareMedians(saved, side([1767.3, 1779.0, 1752.2]))).toMatchObject({
      verdict: 'slower',
      change: expect.closeTo(0.22, 1)
    });
  });

  it('finds a change larger than the spreads of both sides', () => {
    expect(compareMedians(side([660, 664, 660]), side([727, 731, 725]))).toMatchObject({
      verdict: 'slower',
      change: expect.closeTo(0.1, 2)
    });
    expect(compareMedians(side([6.9, 7.1, 6.8]), side([8.8, 8.6, 8.9])).verdict).toBe('slower');
    expect(compareMedians(side([100, 101, 99]), side([90, 91, 89])).verdict).toBe('faster');
    // 4 % apart, but each side scattered by 2 %
    expect(compareMedians(side([100, 102, 100]), side([104, 104, 106])).verdict)
      .toBe('within noise');
  });

  it("can't tell noise from a change with one process on a side, unless it doubles or halves", () => {
    const one = (median: number) => ({ median, spread: 0.01, processes: 1 });
    expect(compareMedians(one(1434), one(1462)).verdict).toBe('one process');
    expect(compareMedians(one(0.10), one(0.16)).verdict).toBe('one process');
    expect(compareMedians(side([100, 101, 99]), one(150)).verdict).toBe('one process');
    expect(compareMedians(one(100), one(210)).verdict).toBe('slower');
    expect(compareMedians(one(100), one(45)).verdict).toBe('faster');
    // --quick: never decides
    expect(compareMedians(one(0.15), one(0.33), Infinity).verdict).toBe('one process');
  });

  it("can't tell when other processes took most of the machine, unless it doubles", () => {
    const quiet = { ...side([3.6, 3.6, 3.6]), otherCpu: 4 };
    const busy = (medians: number[]) => ({ ...side(medians), otherCpu: 80 });
    expect(compareMedians(quiet, busy([3.9, 3.9, 3.9])).verdict).toBe('busy');
    expect(compareMedians(busy([3.6, 3.6, 3.6]), { ...quiet, median: 3.9 }).verdict).toBe('busy');
    expect(compareMedians(quiet, busy([8, 8, 8])).verdict).toBe('slower');
    // a browser and the builds of other work next to it
    expect(compareMedians(quiet, { ...side([3.9, 3.9, 3.9]), otherCpu: 45 }).verdict)
      .toBe('slower');
  });

  it('fails only on a real slowdown above the threshold', () => {
    expect(isRegression(compareMedians(side([100, 101, 99]), side([112, 113, 111])), 0.1))
      .toBe(true);
    expect(isRegression(compareMedians(side([100, 101, 99]), side([108, 109, 107])), 0.1))
      .toBe(false);
    expect(isRegression(compareMedians(side([100, 120, 90]), side([112, 130, 100])), 0.1))
      .toBe(false);
    expect(isRegression(compareMedians(side([100, 101, 99]), side([80, 81, 79])), 0.1)).toBe(false);
    const one = (median: number) => ({ median, spread: 0, processes: 1 });
    expect(isRegression(compareMedians(one(0.10), one(0.16)), 0.1)).toBe(false);
    expect(isRegression(compareMedians(one(100), one(210)), 0.1)).toBe(true);
  });

  it('pairs the cached and the uncached runs by name', () => {
    const run = (name: string, cached: boolean, ms: number) => {
      const process = summarize(name, [{ total: [ms] }, { total: [ms] }], cached);
      return combineProcesses([process, process, process]);
    };
    const comparisons = compareSummaries(
      [run('plant', false, 10), run('plant', true, 1)],
      [run('plant', true, 2), run('plant', false, 10), run('fern', false, 5)]
    );
    expect(comparisons.map(c => [c.name, c.cached, c.total?.verdict])).toEqual([
      ['plant', true, 'slower'],
      ['plant', false, 'within noise'],
      ['fern', false, undefined]
    ]);
  });
});
