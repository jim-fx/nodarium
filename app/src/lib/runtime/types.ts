import type { SerializedNode } from '@nodarium/types';

type RuntimeState = {
  depth: number;
  parents: RuntimeNode[];
  children: RuntimeNode[];
  inputNodes: Record<string, RuntimeNode>;
  // the output of the input node each input is connected to
  inputOutputs: Record<string, number>;
  debugOutput?: number;
};

export type RuntimeNode = null | (SerializedNode & { state: RuntimeState });

/** What a run in the worker sends back */
export type WorkerExecution = {
  result: Int32Array;
  /** the timings of this run */
  performance?: Record<string, number[]>;
  debugData: Record<number, { type: string; output: number; data: Int32Array }>;
  nodeTimings: Record<number, number>;
};
