import { isObject, mergeDeep } from '$lib/helpers/deepMerge';
import { describe, expect, it } from 'vitest';

describe('deepMerge', () => {
  describe('isObject', () => {
    it('should return true for plain objects', () => {
      expect(isObject({})).toBe(true);
      expect(isObject({ a: 1 })).toBe(true);
    });

    it('should return false for non-objects', () => {
      expect(isObject([])).toBe(false);
      expect(isObject('string')).toBe(false);
      expect(isObject(42)).toBe(false);
      expect(isObject(undefined)).toBe(false);
    });
  });

  describe('mergeDeep', () => {
    it('should merge two flat objects', () => {
      const target = { a: 1, b: 2 };
      const source = { b: 3, c: 4 };
      const result = mergeDeep(target, source);

      expect(result).toEqual({ a: 1, b: 3, c: 4 });
    });

    it('should deeply merge nested objects', () => {
      const target = { a: { x: 1 }, b: { y: 2 } };
      const source = { a: { y: 2 }, c: { z: 3 } };
      const result = mergeDeep(target, source);

      expect(result).toEqual({
        a: { x: 1, y: 2 },
        b: { y: 2 },
        c: { z: 3 }
      });
    });

    it('should handle multiple sources', () => {
      const target = { a: 1 };
      const source1 = { b: 2 };
      const source2 = { c: 3 };
      const result = mergeDeep(target, source1, source2);

      expect(result).toEqual({ a: 1, b: 2, c: 3 });
    });

    it('should return target if no sources provided', () => {
      const target = { a: 1 };
      const result = mergeDeep(target);

      expect(result).toBe(target);
    });

    it('should overwrite non-object values', () => {
      const target = { a: { b: 1 } };
      const source = { a: 'string' };
      const result = mergeDeep(target, source);

      expect(result.a).toBe('string');
    });

    it('should handle arrays by replacing', () => {
      const target = { a: [1, 2] };
      const source = { a: [3, 4] };
      const result = mergeDeep(target, source);

      expect(result.a).toEqual([3, 4]);
    });
  });
});
