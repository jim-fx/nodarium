import { describe, expect, it } from 'vitest';
import { fromDisplay, parseLength, roundDisplay, toDisplay } from './units.svelte';

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
});
