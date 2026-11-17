import type { SerializedNode } from '@nodarium/types';

/** What a debug node shows of the output of the node it is connected to */
export type DebugEntry = {
  type: string;
  output: number;
  data: Int32Array;
  /** the share of the items the viewer draws */
  amount?: number;
  /** the share of the length of each path the viewer draws */
  clip?: number;
};

/** What one run of a graph produces */
export type RunResult = {
  result: Int32Array;
  /** keyed by the id of the node a debug node is connected to */
  debugData: Record<number, DebugEntry>;
  /**
   * ms per node, cached nodes keep the time of the run that made their result, group nodes show
   * the sum of their nodes
   */
  nodeTimings: Record<number, number>;
  /** the same, only of the nodes that ran and were not read from the cache */
  runTimings: Record<number, number>;
  /**
   * the bytes of the outputs of the nodes that ran, keyed by the id they ran under: the nodes
   * inside a group by the ids `expandGroups()` gives them
   */
  outputBytes?: Record<number, number>;
};

type RuntimeState = {
  depth: number;
  parents: RuntimeNode[];
  children: RuntimeNode[];
  inputNodes: Record<string, RuntimeNode>;
  // the output of the input node each input is connected to
  inputOutputs: Record<string, number>;
  debugOutput?: number;
};

/** @deprecated the runtime doesn't use it any more, it plans a run without writing into nodes */
export type RuntimeNode = null | (SerializedNode & { state: RuntimeState });
