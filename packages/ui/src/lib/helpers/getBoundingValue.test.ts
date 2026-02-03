import { describe, expect, it } from 'vitest';
import { getBoundingValue } from './getBoundingValue';

describe('getBoundingValue', () => {
  it('should return 1 for values between 0 and 1', () => {
    expect(getBoundingValue(0)).toBe(1);
    expect(getBoundingValue(0.5)).toBe(1);
    expect(getBoundingValue(1)).toBe(1);
  });

  it('should return 2 for values between 1 and 2', () => {
    expect(getBoundingValue(1.1)).toBe(2);
    expect(getBoundingValue(1.5)).toBe(2);
    expect(getBoundingValue(2)).toBe(2);
  });

  it('should return 4 for values between 2 and 4', () => {
    expect(getBoundingValue(2.1)).toBe(4);
    expect(getBoundingValue(3)).toBe(4);
    expect(getBoundingValue(4)).toBe(4);
  });

  it('should return positive level for positive input', () => {
    expect(getBoundingValue(5)).toBe(10);
    expect(getBoundingValue(15)).toBe(20);
    expect(getBoundingValue(50)).toBe(50);
    expect(getBoundingValue(150)).toBe(200);
  });

  it('should return negative level for negative input', () => {
    expect(getBoundingValue(-5)).toBe(-10);
    expect(getBoundingValue(-15)).toBe(-20);
    expect(getBoundingValue(-50)).toBe(-50);
    expect(getBoundingValue(-150)).toBe(-200);
  });

  it('should return correct level for boundary values', () => {
    expect(getBoundingValue(10)).toBe(10);
    expect(getBoundingValue(20)).toBe(20);
    expect(getBoundingValue(50)).toBe(50);
    expect(getBoundingValue(100)).toBe(100);
    expect(getBoundingValue(200)).toBe(200);
  });

  it('should handle large values', () => {
    expect(getBoundingValue(1000)).toBe(1000);
    expect(getBoundingValue(1500)).toBe(1000);
    expect(getBoundingValue(-1000)).toBe(-1000);
  });

  it('should handle very small values', () => {
    expect(getBoundingValue(0.001)).toBe(1);
    expect(getBoundingValue(-0.001)).toBe(-1);
  });
});
