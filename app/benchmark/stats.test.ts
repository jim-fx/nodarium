import { describe, expect, it } from 'vitest';
import { compareSummaries } from './format.ts';
import {
  compareMedians,
  isRegression,
  median,
  percentile,
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

describe('compare', () => {
  it('calls a change real only when it is larger than the spread of both runs', () => {
    expect(compareMedians({ median: 100, spread: 0.02 }, { median: 90, spread: 0.03 }))
      .toMatchObject({ verdict: 'faster', change: expect.closeTo(-0.1) });
    expect(compareMedians({ median: 100, spread: 0.02 }, { median: 103, spread: 0.05 }).verdict)
      .toBe('within noise');
    expect(compareMedians({ median: 100, spread: 0.05 }, { median: 103, spread: 0.01 }).verdict)
      .toBe('within noise');
    expect(compareMedians({ median: 100, spread: 0.01 }, { median: 112, spread: 0.01 }).verdict)
      .toBe('slower');
  });

  it('fails only on a real slowdown above the threshold', () => {
    const slower = (after: number, spread = 0.01) =>
      compareMedians({ median: 100, spread }, { median: after, spread });
    expect(isRegression(slower(112), 0.1)).toBe(true);
    expect(isRegression(slower(108), 0.1)).toBe(false);
    expect(isRegression(slower(112, 0.15), 0.1)).toBe(false);
    expect(isRegression(slower(80), 0.1)).toBe(false);
  });

  it('pairs the cached and the uncached runs by name', () => {
    const run = (name: string, cached: boolean, ms: number) =>
      summarize(name, [{ total: [ms] }, { total: [ms] }, { total: [ms] }], cached);
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
