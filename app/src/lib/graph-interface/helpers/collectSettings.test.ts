import type { NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { collectSettings } from './collectSettings';

function node(id: string, inputs: NodeDefinition['inputs']): NodeDefinition {
  return { id, inputs, outputs: [], meta: {}, execute: () => new Int32Array() } as NodeDefinition;
}

const stem = node('test/plant/stem', {
  resolution: { type: 'integer', value: 16, setting: 'resolution.curve' },
  length: { type: 'float', value: 1 }
});
const output = node('test/plant/output', {
  resolution: { type: 'integer', value: 8, setting: 'resolution.circle' }
});
const noDefault = node('test/plant/nodefault', {
  amount: { type: 'integer', setting: 'amount' }
});

describe('collectSettings', () => {
  it('collects setting types and ignores inputs without a setting', () => {
    const { types } = collectSettings([stem, output]);
    expect(Object.keys(types).sort()).toEqual(['resolution.circle', 'resolution.curve']);
    expect(types['resolution.curve']).toEqual(stem.inputs!.resolution);
  });

  it('fills missing values with defaults and keeps saved values', () => {
    const { values } = collectSettings([stem, output], { 'resolution.curve': 32 });
    expect(values).toEqual({ 'resolution.curve': 32, 'resolution.circle': 8 });
  });

  it('drops saved values no node declares, but keeps the seed', () => {
    const { values } = collectSettings([stem], { 'resolution.circle': 4, 'time.t': 1, seed: 7 });
    expect(values).toEqual({ 'resolution.curve': 16, seed: 7 });
  });

  it('does not add a value for settings without a default', () => {
    const { types, values } = collectSettings([noDefault]);
    expect(types.amount).toBeDefined();
    expect(values).toEqual({});
  });

  it('does not mutate the input values or definitions', () => {
    const saved = { 'resolution.curve': 32 };
    const { types } = collectSettings([stem, output], saved);
    expect(saved).toEqual({ 'resolution.curve': 32 });
    expect(types['resolution.curve']).not.toBe(stem.inputs!.resolution);
  });

  it('handles definitions without inputs', () => {
    expect(collectSettings([node('test/plant/empty', undefined)])).toEqual({
      types: {},
      values: {}
    });
  });
});
