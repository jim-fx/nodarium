const data: Record<string, unknown> = {};

export function clearDebugData() {
  for (const key in data) {
    delete data[key];
  }
}

export function getDebugData() {
  return { ...data };
}

export const debugNode = {
  id: 'max/plantarium/debug',
  inputs: {
    a: {
      type: '*'
    }
  },
  execute(data: Int32Array) {
    return data;
  }
} as const;
