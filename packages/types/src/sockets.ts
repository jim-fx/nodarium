/** Whether an output of type `output` may connect to an input that takes `inputs` */
export function areSocketsCompatible(
  output: string | undefined,
  inputs: string | (string | undefined)[] | undefined
) {
  if (output === '*') return true;
  if (Array.isArray(inputs) && output) {
    return inputs.includes('*') || inputs.includes(output);
  }
  return inputs === output;
}

/** Whether an output of type `output` may connect to `input`, its type or one it `accepts` */
export function inputAccepts(
  input: { type: string; accepts?: string[] } | undefined,
  output: string | undefined
) {
  return areSocketsCompatible(output, [input?.type, ...(input?.accepts ?? [])]);
}
