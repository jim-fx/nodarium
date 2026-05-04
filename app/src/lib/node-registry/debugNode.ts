export const debugNode = {
  id: '__internal/debug/instance',
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
