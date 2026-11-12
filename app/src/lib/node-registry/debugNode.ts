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
    },
    // they travel to the viewer with the debug data, see storeDebugView in the runtime
    amount: {
      type: 'float',
      label: 'Amount',
      value: 1,
      min: 0,
      max: 1,
      step: 0.01,
      hidden: true
    },
    clip: {
      type: 'float',
      label: 'Clip',
      value: 1,
      min: 0,
      max: 1,
      step: 0.01,
      hidden: true
    }
  },
  execute(inputs: NodeValue[]): NodeValue {
    return inputs[0];
  }
} as const;
