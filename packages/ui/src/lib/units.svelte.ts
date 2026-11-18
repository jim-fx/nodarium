import type { InputUnit, ProjectUnit } from '@nodarium/types';
import { evaluate, type Suffixes } from './expression';

/** How many of each unit fit in one meter, values are stored in meters */
export const UNIT_FACTORS: Record<ProjectUnit, number> = {
  m: 1,
  cm: 100,
  mm: 1000
};

export type AngleUnit = 'rad' | 'deg';

/** How many of each angle unit fit in one radian, angles are stored in radians */
export const ANGLE_FACTORS: Record<AngleUnit, number> = {
  rad: 1,
  deg: 180 / Math.PI
};

export const unitSettings = $state<{ unit: ProjectUnit; angle: AngleUnit; show: boolean }>({
  unit: 'm',
  angle: 'rad',
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

const inDegrees = (unit?: InputUnit) => unit === 'rad' && unitSettings.angle === 'deg';

/** How many shown units one stored unit of an input is */
export function unitFactor(unit?: InputUnit): number {
  if (unit === 'm') return UNIT_FACTORS[unitSettings.unit];
  if (unit === 'rad') return ANGLE_FACTORS[unitSettings.angle];
  return 1;
}

export function unitLabel(unit?: InputUnit): string | undefined {
  if (unit === 'm') return unitSettings.unit;
  if (unit === 'rad') return unitSettings.angle === 'deg' ? '°' : 'rad';
}

/**
 * A stored value as an input shows it. Degrees get three decimals, more would only show that a
 * value like 0.8727 was rounded in radians.
 */
export function formatValue(value: number, unit?: InputUnit): number {
  const shown = value * unitFactor(unit);
  return inDegrees(unit) ? +shown.toFixed(3) : roundDisplay(shown);
}

/** The step of an input in shown units, a step in radians becomes a power of ten in degrees */
export function formatStep(step: number, unit?: InputUnit): number {
  const shown = step * unitFactor(unit);
  return inDegrees(unit) ? 10 ** Math.round(Math.log10(shown)) : roundDisplay(shown);
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

function angleSuffixes(fallback: AngleUnit): Suffixes {
  const scale = ANGLE_FACTORS[fallback];
  const degrees = (value: number) => value * scale / ANGLE_FACTORS.deg;
  return { rad: (value) => value * scale, deg: degrees, '°': degrees };
}

/**
 * Evaluates typed text like "45", "45°", "0.785rad" or "pi / 4" to radians, a number without a
 * unit is read in the angle unit, and pi is half a turn in both.
 */
export function parseAngle(
  text: string,
  fallback: AngleUnit = unitSettings.angle
): number | undefined {
  const scale = ANGLE_FACTORS[fallback];
  const result = evaluate(text, angleSuffixes(fallback), Math.PI * scale);
  return result === undefined ? undefined : result / scale;
}

/**
 * Typed text as the stored value of an input. An input without a unit takes "30°" as radians too,
 * for a random or a float that feeds an angle.
 */
export function parseValue(text: string, unit?: InputUnit): number | undefined {
  if (unit === 'm') return parseLength(text);
  if (unit === 'rad') return parseAngle(text);
  return evaluate(text, angleSuffixes('rad'));
}

/**
 * Shows the angles of a description in the angle unit: a description writes them as "1.5708 rad"
 * and says "in radians", so the text reads the same as the fields.
 */
export function describeAngles(text: string, angle: AngleUnit = unitSettings.angle): string {
  if (angle === 'rad') return text;
  return text
    .replace(
      /(-?\d+(?:\.\d+)?) rad\b/g,
      (_, value: string) => `${+(Number(value) * ANGLE_FACTORS.deg).toFixed(1)}°`
    )
    .replaceAll('in radians', 'in degrees');
}
