export const groupNode = {
  id: '__internal/group/instance',
  meta: { title: 'Group' },
  inputs: {
    input: {
      type: 'select',
      values: []
    }
  },
  execute(_data: Int32Array): Int32Array {
    return _data;
  }
} as const;
