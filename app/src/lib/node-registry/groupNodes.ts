import type { NodeDefinition } from '@nodarium/types';

export const groupInputNode: NodeDefinition = {
  id: '__virtual/group/input',
  inputs: {},
  outputs: [],
  execute(_data: Int32Array): Int32Array { return _data; }
} as unknown as NodeDefinition;

export const groupOutputNode: NodeDefinition = {
  id: '__virtual/group/output',
  inputs: {},
  outputs: [],
  execute(_data: Int32Array): Int32Array { return _data; }
} as unknown as NodeDefinition;

// Stub registered in the registry so it appears in AddMenu.
// Actual inputs/outputs are resolved from props.groupId at runtime.
export const groupNode: NodeDefinition = {
  id: '__virtual/group/instance',
  meta: { title: 'Group' },
  inputs: {},
  outputs: [],
  execute(_data: Int32Array): Int32Array { return _data; }
} as unknown as NodeDefinition;
