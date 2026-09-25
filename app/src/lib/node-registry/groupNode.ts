import type { NodeValue } from '@nodarium/types';

export const groupNode = {
  id: '__internal/group/instance',
  meta: { title: 'Group' },
  inputs: {
    groupId: {
      label: '',
      type: 'select',
      values: []
    }
  },
  execute(inputs: NodeValue[]): NodeValue {
    return inputs[0];
  }
} as const;
