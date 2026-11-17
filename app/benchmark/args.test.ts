import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import {
  parseBenchArgs,
  parseProfileArgs,
  parseProfileNodeArgs,
  parseScale,
  runsFor,
  UsageError,
  userPath
} from './args.ts';
import { demangle, topFunctions } from './cpu-profile.ts';
import { argsFileName, decodeArgs, encodeArgs } from './node-args.ts';
import { resolveTarget, scaleProps } from './targets.ts';

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

  it('scales an input of every node of a type, in groups too, whole numbers stay whole', () => {
    const graph = {
      id: 0,
      nodes: [
        { id: 1, type: 'max/plantarium/instance', props: { amount: 3, scale: 0.5 } },
        { id: 2, type: 'max/plantarium/instance', props: {} },
        { id: 3, type: 'max/plantarium/stem', props: { amount: 3 } }
      ],
      edges: [],
      groups: [{
        id: 1,
        nodes: [{ id: 1, type: 'max/plantarium/instance', props: { amount: 5 } }],
        edges: []
      }]
    } as unknown as Graph;
    const changed = scaleProps(
      graph,
      [{ type: 'instance', prop: 'amount', factor: 1.5 }, {
        type: 'max/plantarium/instance',
        prop: 'scale',
        factor: 3
      }],
      (type, prop) => (prop === 'amount' ? 2 : undefined)
    );
    expect(changed).toEqual([3, 1]);
    expect(graph.nodes.map(n => n.props)).toEqual([
      { amount: 5, scale: 1.5 },
      { amount: 3 },
      { amount: 3 }
    ]);
    expect(graph.groups![0].nodes[0].props).toEqual({ amount: 8 });
  });
});

describe('pnpm profile', () => {
  it('reads targets and flags', () => {
    expect(parseProfileArgs(['porcini'])).toMatchObject({
      target: 'porcini',
      seed: 1,
      builds: 3,
      cache: false,
      set: [],
      scale: []
    });
    expect(
      parseProfileArgs([
        'fern',
        '--seed',
        '7',
        '--builds',
        '1',
        '--cache',
        '--set',
        'resolution.curve=8',
        '--set',
        '4.length=0.3',
        '--scale',
        'max/plantarium/instance.amount=4',
        '--dump-args',
        'args'
      ])
    ).toMatchObject({
      target: 'fern',
      seed: 7,
      builds: 1,
      cache: true,
      set: ['resolution.curve=8', '4.length=0.3'],
      scale: [{ type: 'max/plantarium/instance', prop: 'amount', factor: 4 }],
      dumpArgs: 'args'
    });
    expect(() => parseProfileArgs(['fern', 'porcini'])).toThrow(UsageError);
    expect(() => parseProfileArgs(['fern', '--builds', '0'])).toThrow(UsageError);
  });

  it('reads a scale as type.prop=factor', () => {
    expect(parseScale('instance.amount=0.5')).toEqual({
      type: 'instance',
      prop: 'amount',
      factor: 0.5
    });
    expect(() => parseScale('amount=2')).toThrow(UsageError);
    expect(() => parseScale('instance.amount=lots')).toThrow(UsageError);
  });

  it('reads the flags of profile:node', () => {
    expect(parseProfileNodeArgs(['strands', 'a.args', '--runs', '3', '--cpu-prof'])).toEqual({
      node: 'strands',
      args: 'a.args',
      runs: 3,
      wasm: undefined,
      cpuProf: true,
      help: false
    });
  });

  it('writes the inputs of a node and reads them back', () => {
    const inputs = [Int32Array.of(1, 2, 3), new Int32Array(0), Int32Array.of(-7)];
    const bytes = encodeArgs(inputs);
    // as Node reads a small file: a Buffer in a larger pool, its slice no copy
    const pooled = Buffer.alloc(bytes.byteLength + 12);
    pooled.set(bytes, 12);
    expect(decodeArgs(pooled.subarray(12))).toEqual(inputs);
    expect(() => decodeArgs(new Uint8Array(Int32Array.of(2, 5, 1, 1).buffer))).toThrow();
    expect(argsFileName('max/myzel/strands', 2)).toBe('max_myzel_strands.2.args');
  });

  it('names the functions of a cpu profile of a release node', () => {
    expect(demangle('_RNvMs0_NtCshrbwMYXEhZD_10myzel_core7densityNtB5_12ClosedSearch5block'))
      .toBe('myzel_core::density::ClosedSearch::block');
    expect(demangle('_RNvNtCshrbwMYXEhZD_10myzel_core7density14transform_line'))
      .toBe('myzel_core::density::transform_line');
    expect(demangle('cbrtf')).toBe('cbrtf');

    const frame = (id: number, functionName: string, url = 'wasm://wasm/abc') => ({
      id,
      callFrame: { functionName, url }
    });
    const { total, top } = topFunctions({
      nodes: [frame(1, 'a'), frame(2, 'b'), frame(3, '(garbage collector)', '')],
      samples: [1, 2, 1, 3, 1],
      timeDeltas: [1000, 1000, 1000, 1000, 1000]
    }, 2);
    expect(total).toBe(5);
    expect(top).toEqual([{ name: 'a', ms: 3, share: 0.6 }, { name: 'b', ms: 1, share: 0.2 }]);
  });
});
