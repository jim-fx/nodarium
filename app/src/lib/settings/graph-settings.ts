import { type NodeInput, type ProjectUnit, ProjectUnitSchema } from '@nodarium/types';
import { UNIT_FACTORS } from '@nodarium/ui';

// a key no node can declare as a setting, they are named like "resolution.circle"
export const UNIT_SETTING = 'meta.unit';

/**
 * What the Graph Settings panel shows: the unit of the project first, then the settings of the
 * graph. The unit lives in the meta of the graph, not in its settings.
 */
export function graphPanelSettings(
  types: Record<string, NodeInput>,
  values: Record<string, unknown>,
  unit: ProjectUnit = 'm'
): { types: Record<string, NodeInput>; values: Record<string, unknown> } {
  return {
    types: {
      [UNIT_SETTING]: {
        type: 'select',
        label: 'Unit',
        options: Object.keys(UNIT_FACTORS),
        value: 'm'
      },
      ...types
    },
    values: { [UNIT_SETTING]: unit, ...values }
  };
}

/** Sends a change of the Graph Settings panel to the project, for the unit, or to the graph */
export function changeGraphPanelSetting(
  key: string,
  value: unknown,
  to: { unit: (unit: ProjectUnit) => void; setting: (key: string, value: unknown) => void }
) {
  if (key !== UNIT_SETTING) {
    to.setting(key, value);
    return;
  }
  const unit = ProjectUnitSchema.safeParse(value);
  if (unit.success) to.unit(unit.data);
}
