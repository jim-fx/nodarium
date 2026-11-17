import { describe, expect, it } from 'vitest';
import { parseBenchArgs, runsFor, UsageError, userPath } from './args.ts';
import { resolveTarget } from './targets.ts';

describe('pnpm bench', () => {
  it('runs what CI runs without arguments', () => {
    expect(parseBenchArgs([])).toMatchObject({
      targets: [],
      cached: true,
      quick: false,
      threshold: 0.1,
      json: false
    });
  });

  it('reads targets and flags', () => {
    const options = parseBenchArgs([
      'plant',
      'fern',
      '--runs',
      '20',
      '--no-cached',
      '--save',
      'before',
      '--compare',
      '../data/main',
      '--threshold',
      '5',
      '--json'
    ]);
    expect(options).toMatchObject({
      targets: ['plant', 'fern'],
      runs: 20,
      cached: false,
      save: 'before',
      compare: '../data/main',
      threshold: 0.05,
      json: true
    });
  });

  it('refuses what it can not use', () => {
    expect(() => parseBenchArgs(['--runs', '0'])).toThrow(UsageError);
    expect(() => parseBenchArgs(['--runs', 'many'])).toThrow(UsageError);
    expect(() => parseBenchArgs(['--save', '../x'])).toThrow(UsageError);
    expect(() => parseBenchArgs(['--fast'])).toThrow(UsageError);
    expect(() => parseBenchArgs(['--threshold', '-1'])).toThrow(UsageError);
  });

  it('takes a tenth of the runs with --quick, at least 3, unless they are given', () => {
    const plant = { runs: 100, warmups: 10 };
    const mushroom = { runs: 8, warmups: 1 };
    expect(runsFor(plant, { quick: true })).toEqual({ runs: 10, warmups: 1 });
    expect(runsFor(mushroom, { quick: true })).toEqual({ runs: 3, warmups: 1 });
    expect(runsFor(plant, { quick: false })).toEqual(plant);
    expect(runsFor(plant, { quick: true, runs: 40, warmups: 0 })).toEqual({ runs: 40, warmups: 0 });
  });

  it('reads paths relative to where pnpm was started', () => {
    expect(userPath('graph.json', '/home/me/nodarium')).toBe('/home/me/nodarium/graph.json');
    expect(userPath('/tmp/graph.json', '/home/me/nodarium')).toBe('/tmp/graph.json');
  });
});

describe('targets', () => {
  const benchNames = ['plant', 'porcini'];
  const exists = (path: string) => path === '/work/plant.json' || path === '/work/porcini';

  it('takes a template of benchmark/templates first, then a file, then a template id', () => {
    expect(resolveTarget('plant', benchNames, exists, '/work')).toEqual({
      kind: 'bench',
      name: 'plant'
    });
    expect(resolveTarget('porcini', benchNames, exists, '/work').kind).toBe('bench');
    expect(resolveTarget('plant.json', benchNames, exists, '/work')).toEqual({
      kind: 'source',
      target: '/work/plant.json'
    });
    expect(resolveTarget('fern', benchNames, exists, '/work')).toEqual({
      kind: 'source',
      target: 'fern'
    });
  });
});
