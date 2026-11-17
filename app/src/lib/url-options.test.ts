import { describe, expect, it } from 'vitest';
import { readUrlOptions } from './url-options';

describe('readUrlOptions', () => {
  it('lets the tutorial start without parameters', () => {
    expect(readUrlOptions('')).toEqual({ tutorial: true });
  });

  it('keeps the tutorial closed with tutorial=0', () => {
    expect(readUrlOptions('?tutorial=0').tutorial).toBe(false);
    expect(readUrlOptions('?foo=1&tutorial=0').tutorial).toBe(false);
  });

  it('takes any other value as the default', () => {
    expect(readUrlOptions('?tutorial=1').tutorial).toBe(true);
    expect(readUrlOptions('?tutorial').tutorial).toBe(true);
  });
});
