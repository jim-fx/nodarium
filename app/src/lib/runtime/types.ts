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
