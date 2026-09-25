import type { NodeValue } from '@nodarium/types';

export const debugNode = {
  id: '__internal/node/debug',
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
