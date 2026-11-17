import type { ProjectUnit } from '@nodarium/types';
import { evaluate } from './expression';

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

/**
 * Evaluates typed text like "10", "2,5cm", "300 mm" or "10cm + 5mm * 2" to meters,
 * a number without a unit is read in the project unit. Returns undefined if it is not a length.
 */
export function parseLength(
  text: string,
  fallback: ProjectUnit = unitSettings.unit
): number | undefined {
  // every number is brought to the fallback unit first, so "10cm * 2" is 20 cm in any project unit
  const scale = UNIT_FACTORS[fallback];
  const suffixes = Object.fromEntries(
    Object.entries(UNIT_FACTORS).map((
      [unit, factor]
    ) => [unit, (value: number) => value * scale / factor])
  );
  const result = evaluate(text, suffixes);
  return result === undefined ? undefined : fromDisplay(result, fallback);
}
