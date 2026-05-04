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
  execute(_data: Int32Array): Int32Array {
    return _data;
  }
} as const;
