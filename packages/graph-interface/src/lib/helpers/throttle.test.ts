import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import throttle from './throttle.ts';

describe('throttle', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('runs the first call immediately', () => {
    const cb = vi.fn();
    throttle(cb, 10)(1);
    expect(cb).toHaveBeenCalledExactlyOnceWith(1);
  });

  it('runs one trailing call with the latest arguments', () => {
    const cb = vi.fn();
    const t = throttle(cb, 10);
    t(1);
    t(2);
    t(3);
    expect(cb).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(10);
    expect(cb).toHaveBeenCalledTimes(2);
    expect(cb).toHaveBeenLastCalledWith(3);
    vi.advanceTimersByTime(50);
    expect(cb).toHaveBeenCalledTimes(2);
  });

  it('runs immediately again once the window has passed', () => {
    const cb = vi.fn();
    const t = throttle(cb, 10);
    t(1);
    vi.advanceTimersByTime(10);
    t(2);
    expect(cb).toHaveBeenCalledTimes(2);
    expect(cb).toHaveBeenLastCalledWith(2);
  });
});
