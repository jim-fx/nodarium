import type { SerializedNode } from '@nodarium/types';

type RuntimeState = {
  depth: number;
  parents: RuntimeNode[];
  children: RuntimeNode[];
  inputNodes: Record<string, RuntimeNode>;
};

export type RuntimeNode = SerializedNode & { state: RuntimeState };
