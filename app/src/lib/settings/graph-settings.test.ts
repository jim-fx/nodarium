import type { NodeInput } from '@nodarium/types';
import { describe, expect, it, vi } from 'vitest';
import { changeGraphPanelSetting, graphPanelSettings, UNIT_SETTING } from './graph-settings';

const types: Record<string, NodeInput> = {
  seed: { type: 'integer', value: 0 },
  'resolution.circle': { type: 'integer', value: 8 }
};

describe('graphPanelSettings', () => {
  it('puts the unit of the project before the settings of the graph', () => {
    const panel = graphPanelSettings(types, { seed: 3, 'resolution.circle': 12 }, 'cm');
    expect(Object.keys(panel.types)).toEqual([UNIT_SETTING, 'seed', 'resolution.circle']);
    expect(panel.types[UNIT_SETTING]).toMatchObject({ type: 'select', options: ['m', 'cm', 'mm'] });
    expect(panel.values).toEqual({ [UNIT_SETTING]: 'cm', seed: 3, 'resolution.circle': 12 });
  });

  it('shows meters for a graph without a unit', () => {
    expect(graphPanelSettings(types, {}).values[UNIT_SETTING]).toBe('m');
  });
});

describe('changeGraphPanelSetting', () => {
  it('sends the unit to the project and the rest to the graph', () => {
    const to = { unit: vi.fn(), setting: vi.fn() };
    changeGraphPanelSetting(UNIT_SETTING, 'mm', to);
    changeGraphPanelSetting('resolution.circle', 16, to);
    expect(to.unit).toHaveBeenCalledExactlyOnceWith('mm');
    expect(to.setting).toHaveBeenCalledExactlyOnceWith('resolution.circle', 16);
  });

  it('ignores a unit the project does not know', () => {
    const to = { unit: vi.fn(), setting: vi.fn() };
    changeGraphPanelSetting(UNIT_SETTING, 'km', to);
    expect(to.unit).not.toHaveBeenCalled();
    expect(to.setting).not.toHaveBeenCalled();
  });
});
