import type { NodeValue } from '@nodarium/types';
import { GROUP_INSTANCE } from './internal-nodes';

export const groupNode = {
  id: GROUP_INSTANCE,
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
