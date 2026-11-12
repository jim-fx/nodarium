import type { PerformanceData } from '@nodarium/utils';

type Run = PerformanceData[number];

/**
 * A run holds one sample per node for keys like `node/<type>`, `collected-inputs` or
 * `cache-hit`, so the first sample alone is only the first node. Timings add up over the run,
 * `cache-hit` is the share of nodes served from the cache.
 */
export function runValue(run: Run | undefined, key: string): number | undefined {
  const samples = run?.[key];
  if (!samples?.length) return undefined;
  const sum = samples.reduce((acc, v) => acc + v, 0);
  return key === 'cache-hit' ? sum / samples.length : sum;
}

/**
 * Runs the same input only once in a row, a repeat would give the same result. The key is
 * forgotten when a run fails, so the input can be retried.
 */
export function createRepeatGuard() {
  let last: string | undefined;
  return {
    isRepeat: (key: string) => key === last,
    succeeded: (key: string) => {
      last = key;
    },
    failed: () => {
      last = undefined;
    }
  };
}
