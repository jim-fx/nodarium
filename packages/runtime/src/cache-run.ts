import type { NodeValue, SyncCache } from '@nodarium/types';
import { readNodeValue } from '@nodarium/utils';
import { byteSize } from './runtime-executor-cache';

type CacheRun = {
  readonly enabled: boolean;
  read(key: string): Int32Array[] | undefined;
  /** Copies and stores the values when the cache keeps them */
  store(key: string, values: NodeValue[], cost: number): void;
  /** Sets the limits to twice this run */
  finish(): void;
};

const NO_CACHE: CacheRun = {
  enabled: false,
  read: () => undefined,
  store() {},
  finish() {}
};

function nodeValueBytes(value: NodeValue) {
  return value instanceof Int32Array ? value.byteLength : value.len * 4;
}

/** The cache as one run sees it, a run without a cache gets one that stores nothing */
export function startCacheRun(
  cache: SyncCache<Int32Array[]> | undefined,
  nodeCount: number
): CacheRun {
  if (!cache) return NO_CACHE;

  cache.startRun?.();
  // a bigger graph than the last one keeps all of its results
  cache.size = Math.max(cache.size, nodeCount * 2);
  let bytes = 0;

  return {
    enabled: true,
    read(key) {
      const values = cache.get(key);
      if (values !== undefined) bytes += byteSize(values);
      return values;
    },
    store(key, values, cost) {
      // asked before the values are copied out of the memory of their node
      const valueBytes = values.reduce((sum, value) => sum + nodeValueBytes(value), 0);
      if (!(cache.keeps?.(valueBytes, cost) ?? true)) return;
      bytes += valueBytes;
      // a bigger graph than the last one keeps all of its results
      cache.maxBytes = Math.max(cache.maxBytes ?? 0, bytes * 2);
      cache.set(key, values.map(readNodeValue), cost);
    },
    // every change of a value or the seed adds new results, the cache keeps those of this run and
    // as many older ones again, so a change can be undone without running the nodes again
    finish() {
      cache.size = nodeCount * 2;
      cache.maxBytes = bytes * 2;
    }
  };
}
