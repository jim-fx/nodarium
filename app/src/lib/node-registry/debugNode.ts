export const debugNode = {
  id: '__internal/debug/instance',
  inputs: {
    input: {
      type: '*'
    }
  },
  execute(_data: Int32Array): Int32Array {
    return _data;
  }
} as const;
