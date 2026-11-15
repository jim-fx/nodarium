import type { NodeDefinition, NodeInput } from '@nodarium/types';

/** The settings every graph has, no node declares them. The runtime hands the seed to every seed input. */
export const GRAPH_SETTING_TYPES: Record<string, NodeInput> = {
  seed: { type: 'integer', label: 'Seed', value: 0 }
};

/** The settings of a graph with these nodes: their types, and the values of them, nothing else */
export function collectSettings(
  definitions: NodeDefinition[],
  values: Record<string, unknown> = {}
): { types: Record<string, NodeInput>; values: Record<string, unknown> } {
  const types: Record<string, NodeInput> = {};
  for (const key in GRAPH_SETTING_TYPES) types[key] = { ...GRAPH_SETTING_TYPES[key] };
  for (const definition of definitions) {
    for (const key in definition.inputs) {
      const input = definition.inputs[key];
      const settingId = input.setting;
      if (!settingId) continue;
      types[settingId] = { ...input };
    }
  }
  const filled: Record<string, unknown> = {};
  for (const settingId in types) {
    const input = types[settingId];
    if (values[settingId] !== undefined) filled[settingId] = values[settingId];
    else if ('value' in input) filled[settingId] = input.value;
  }
  return { types, values: filled };
}
