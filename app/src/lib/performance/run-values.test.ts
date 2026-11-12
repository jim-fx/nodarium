import { describe, expect, it } from 'vitest';
import { createRepeatGuard, runValue } from './run-values';

describe('runValue', () => {
  it('sums the samples of every node in a run', () => {
    const run = { 'node/max/myzel/stem': [4, 6], runtime: [2058] };
    expect(runValue(run, 'node/max/myzel/stem')).toBe(10);
    expect(runValue(run, 'runtime')).toBe(2058);
  });

  it('gives the share of cache hits', () => {
    expect(runValue({ 'cache-hit': [1, 1, 0, 0] }, 'cache-hit')).toBe(0.5);
  });

  it('is undefined for a key the run does not have', () => {
    expect(runValue({ runtime: [1] }, 'update-geometries')).toBeUndefined();
    expect(runValue({ runtime: [] }, 'runtime')).toBeUndefined();
    expect(runValue(undefined, 'runtime')).toBeUndefined();
  });
});

describe('createRepeatGuard', () => {
  it('skips the same input after it ran', () => {
    const guard = createRepeatGuard();
    expect(guard.isRepeat('a')).toBe(false);
    guard.succeeded('a');
    expect(guard.isRepeat('a')).toBe(true);
    expect(guard.isRepeat('b')).toBe(false);
  });

  it('runs an input again after a failure', () => {
    const guard = createRepeatGuard();
    guard.succeeded('a');
    guard.failed();
    expect(guard.isRepeat('a')).toBe(false);
  });
});
