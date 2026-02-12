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
    input: {
      type: '*'
    }
  },
  execute() {}
} as const;
