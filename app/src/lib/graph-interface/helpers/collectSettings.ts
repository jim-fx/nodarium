import type { NodeDefinition, NodeInput } from '@nodarium/types';

export function collectSettings(
  definitions: NodeDefinition[],
  values: Record<string, unknown> = {}
): { types: Record<string, NodeInput>; values: Record<string, unknown> } {
  const types: Record<string, NodeInput> = {};
  const filled = { ...values };
  for (const definition of definitions) {
    for (const key in definition.inputs) {
      const input = definition.inputs[key];
      const settingId = input.setting;
      if (!settingId) continue;
      types[settingId] = { ...input };
      if (filled[settingId] === undefined && 'value' in input) {
        filled[settingId] = input.value;
      }
    }
  }
  return { types, values: filled };
}
