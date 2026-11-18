import { describe, expect, it } from 'vitest';
import {
  describeAngles,
  formatStep,
  formatValue,
  fromDisplay,
  parseAngle,
  parseLength,
  parseValue,
  roundDisplay,
  toDisplay,
  unitSettings
} from './units.svelte';

describe('units', () => {
  it('converts meters to the display unit', () => {
    expect(toDisplay(1.5, 'm')).toBe(1.5);
    expect(toDisplay(0.25, 'cm')).toBe(25);
    expect(toDisplay(0.002, 'mm')).toBe(2);
  });

  it('round trips', () => {
    for (const unit of ['m', 'cm', 'mm'] as const) {
      expect(roundDisplay(fromDisplay(toDisplay(0.123, unit), unit))).toBe(0.123);
    }
  });

  it('removes float noise', () => {
    expect(roundDisplay(0.1 * 3)).toBe(0.3);
  });

  it('parses typed lengths', () => {
    expect(parseLength('10m', 'cm')).toBe(10);
    expect(parseLength('10 cm', 'm')).toBeCloseTo(0.1);
    expect(parseLength('5mm', 'm')).toBeCloseTo(0.005);
    expect(parseLength('2,5', 'cm')).toBeCloseTo(0.025);
    expect(parseLength('abc', 'm')).toBeUndefined();
  });

  it('calculates with typed lengths', () => {
    expect(parseLength('10cm + 5mm', 'm')).toBeCloseTo(0.105);
    expect(parseLength('10cm + 5', 'mm')).toBeCloseTo(0.105);
    expect(parseLength('10cm * 2', 'm')).toBeCloseTo(0.2);
    expect(parseLength('1m / 4', 'cm')).toBeCloseTo(0.25);
    expect(parseLength('2 * 3', 'cm')).toBeCloseTo(0.06);
    expect(parseLength('pi', 'cm')).toBeCloseTo(Math.PI / 100);
  });

  describe('angles', () => {
    function inDegrees(run: () => void) {
      unitSettings.angle = 'deg';
      try {
        run();
      } finally {
        unitSettings.angle = 'rad';
      }
    }

    it('shows radians as they are by default', () => {
      expect(formatValue(0.8727, 'rad')).toBe(0.8727);
      expect(formatStep(0.01, 'rad')).toBe(0.01);
    });

    it('shows radians in degrees', () => {
      inDegrees(() => {
        expect(formatValue(Math.PI / 2, 'rad')).toBe(90);
        expect(formatValue(0.8727, 'rad')).toBe(50.002);
        expect(formatStep(0.01, 'rad')).toBe(1);
        expect(formatStep(0.05, 'rad')).toBe(1);
        expect(formatValue(0.25, 'm')).toBe(0.25);
        expect(formatValue(0.25)).toBe(0.25);
      });
    });

    it('reads a typed degree back as typed, without drift', () => {
      inDegrees(() => {
        for (const degrees of [0, 1, 15, 30, 45, 60, 90, 137.5, 180, 270, 359.9, -45]) {
          const stored = parseAngle(String(degrees))!;
          expect(formatValue(stored, 'rad')).toBe(degrees);
          expect(formatValue(parseAngle(String(formatValue(stored, 'rad')))!, 'rad')).toBe(degrees);
        }
      });
    });

    it('parses typed angles in either unit', () => {
      expect(parseAngle('45', 'deg')).toBeCloseTo(Math.PI / 4);
      expect(parseAngle('0.5', 'rad')).toBe(0.5);
      expect(parseAngle('45°', 'rad')).toBeCloseTo(Math.PI / 4);
      expect(parseAngle('45 deg', 'rad')).toBeCloseTo(Math.PI / 4);
      expect(parseAngle('0.785rad', 'deg')).toBeCloseTo(0.785);
      expect(parseAngle('pi / 2', 'deg')).toBeCloseTo(Math.PI / 2);
      expect(parseAngle('pi / 2', 'rad')).toBeCloseTo(Math.PI / 2);
      expect(parseAngle('90° + 0.5rad', 'deg')).toBeCloseTo(Math.PI / 2 + 0.5);
      expect(parseAngle('45 * 2', 'deg')).toBeCloseTo(Math.PI / 2);
      expect(parseAngle('abc', 'deg')).toBeUndefined();
    });

    it('takes a typed angle as radians in a field without a unit', () => {
      expect(parseValue('30°')).toBeCloseTo(Math.PI / 6);
      expect(parseValue('30')).toBe(30);
    });

    it('shows the angles of a description in degrees', () => {
      const text = 'Tilts it, in radians: 1.5708 rad along it, 2.3998 rad is the golden angle.';
      expect(describeAngles(text, 'rad')).toBe(text);
      expect(describeAngles(text, 'deg')).toBe(
        'Tilts it, in degrees: 90° along it, 137.5° is the golden angle.'
      );
      expect(describeAngles('bends 1.5 m, 0 = flat', 'deg')).toBe('bends 1.5 m, 0 = flat');
    });
  });
});
