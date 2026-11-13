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
