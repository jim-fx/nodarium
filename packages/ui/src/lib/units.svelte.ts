import type { ProjectUnit } from '@nodarium/types';

/** How many of each unit fit in one meter, values are stored in meters */
export const UNIT_FACTORS: Record<ProjectUnit, number> = {
  m: 1,
  cm: 100,
  mm: 1000
};

export const unitSettings = $state<{ unit: ProjectUnit; show: boolean }>({
  unit: 'm',
  show: true
});

export function toDisplay(meters: number, unit: ProjectUnit = unitSettings.unit): number {
  return meters * UNIT_FACTORS[unit];
}

export function fromDisplay(value: number, unit: ProjectUnit = unitSettings.unit): number {
  return value / UNIT_FACTORS[unit];
}

/** Removes float noise like 0.30000000000000004 from converted values */
export function roundDisplay(value: number): number {
  return +value.toPrecision(10);
}
