import type { NodeDefinition, NodeInput } from '@nodarium/types';

// no node declares the seed, the runtime hands it to every seed input
const GRAPH_SETTINGS = ['seed'];

/** The values of the declared settings and of the graph's own settings, nothing else */
export function pickSettings(
  types: Record<string, unknown>,
  values: Record<string, unknown> = {}
): Record<string, unknown> {
  const picked: Record<string, unknown> = {};
  for (const key in values) {
    if (key in types || GRAPH_SETTINGS.includes(key)) picked[key] = values[key];
  }
  return picked;
}

export function collectSettings(
  definitions: NodeDefinition[],
  values: Record<string, unknown> = {}
): { types: Record<string, NodeInput>; values: Record<string, unknown> } {
  const types: Record<string, NodeInput> = {};
  for (const definition of definitions) {
    for (const key in definition.inputs) {
      const input = definition.inputs[key];
      const settingId = input.setting;
      if (!settingId) continue;
      types[settingId] = { ...input };
    }
  }
  const filled = pickSettings(types, values);
  for (const settingId in types) {
    const input = types[settingId];
    if (filled[settingId] === undefined && 'value' in input) {
      filled[settingId] = input.value;
    }
  }
  return { types, values: filled };
}
