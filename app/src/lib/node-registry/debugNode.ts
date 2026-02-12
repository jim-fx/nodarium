export const debugNode = {
  id: 'max/plantarium/debug',
  inputs: {
    input: {
      type: '*'
    }
  },
  execute(_data: Int32Array): Int32Array {
    return _data;
  }
} as const;
