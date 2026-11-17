import { describe, expect, it } from 'vitest';
import { evaluate } from './expression';

describe('evaluate', () => {
  it('reads plain numbers', () => {
    expect(evaluate('42')).toBe(42);
    expect(evaluate(' -1.5 ')).toBe(-1.5);
    expect(evaluate('2,5')).toBe(2.5);
    expect(evaluate('.5')).toBe(0.5);
    expect(evaluate('1e-3')).toBe(0.001);
  });

  it('adds, subtracts, multiplies and divides', () => {
    expect(evaluate('1 + 2')).toBe(3);
    expect(evaluate('5-3')).toBe(2);
    expect(evaluate('4 * 2.5')).toBe(10);
    expect(evaluate('9 / 2')).toBe(4.5);
  });

  it('multiplies before it adds, from left to right', () => {
    expect(evaluate('1 + 2 * 3')).toBe(7);
    expect(evaluate('10 - 4 - 3')).toBe(3);
    expect(evaluate('16 / 4 / 2')).toBe(2);
  });

  it('reads parentheses and signs', () => {
    expect(evaluate('(1 + 2) * 3')).toBe(9);
    expect(evaluate('2 * -3')).toBe(-6);
    expect(evaluate('-(2 + 3)')).toBe(-5);
    expect(evaluate('5 - -3')).toBe(8);
  });

  it('knows pi', () => {
    expect(evaluate('pi')).toBe(Math.PI);
    expect(evaluate('PI')).toBe(Math.PI);
    expect(evaluate('2 * pi')).toBe(2 * Math.PI);
    expect(evaluate('-pi / 2')).toBe(-Math.PI / 2);
  });

  it('converts a number with a suffix', () => {
    const suffixes = { k: (value: number) => value * 1000 };
    expect(evaluate('2k', suffixes)).toBe(2000);
    expect(evaluate('2 k + 5', suffixes)).toBe(2005);
    expect(evaluate('2k')).toBeUndefined();
    expect(evaluate('2constructor')).toBeUndefined();
  });

  it('returns undefined for text that is not an expression', () => {
    for (const text of ['', 'abc', '1 +', '* 2', '(1 + 2', '1 2', '2pi', '1 / 0', '1e']) {
      expect(evaluate(text), text).toBeUndefined();
    }
  });
});
