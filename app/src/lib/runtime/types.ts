import type { SerializedNode } from '@nodarium/types';

type RuntimeState = {
  depth: number;
  parents: RuntimeNode[];
  children: RuntimeNode[];
  inputNodes: Record<string, RuntimeNode>;
  debugNode?: boolean;
};

export type RuntimeNode = null | (SerializedNode & { state: RuntimeState });
