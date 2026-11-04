import type { NodeValue } from '@nodarium/types';
import { DEBUG_NODE } from '@nodarium/types';

export const debugNode = {
  id: DEBUG_NODE,
  meta: {
    title: 'Debug'
  },
  inputs: {
    input: {
      type: '*',
      label: ''
    }
  },
  execute(inputs: NodeValue[]): NodeValue {
    return inputs[0];
  }
} as const;
