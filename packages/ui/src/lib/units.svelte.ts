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

const LENGTH_PATTERN = /^(-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?)\s*(mm|cm|m)?$/i;

/**
 * Parses typed text like "10", "2,5cm" or "300 mm" to meters,
 * a number without a unit is read in the project unit. Returns undefined if it is not a length.
 */
export function parseLength(
  text: string,
  fallback: ProjectUnit = unitSettings.unit
): number | undefined {
  const match = LENGTH_PATTERN.exec(text.trim().replace(',', '.'));
  if (!match) return undefined;
  const unit = (match[2]?.toLowerCase() as ProjectUnit | undefined) ?? fallback;
  return fromDisplay(parseFloat(match[1]), unit);
}
