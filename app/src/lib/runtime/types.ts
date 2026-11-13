import type { DebugEntry } from '@nodarium/runtime';

/** What a run in the worker sends back */
export type WorkerExecution = {
  result: Int32Array;
  /** the timings of this run */
  performance?: Record<string, number[]>;
  debugData: Record<number, DebugEntry>;
  nodeTimings: Record<number, number>;
};
