import { debounceAsyncFunction, humanizeDuration, humanizeNumber } from '#lib/helpers/index.ts';
import { describe, expect, it, vi } from 'vitest';

describe('helpers', () => {
  describe('humanizeNumber', () => {
    it('should return unchanged numbers below 1000', () => {
      expect(humanizeNumber(0)).toBe('0');
      expect(humanizeNumber(999)).toBe('999');
    });

    it('should add K suffix for thousands', () => {
      expect(humanizeNumber(1000)).toBe('1K');
      expect(humanizeNumber(1500)).toBe('1.5K');
      expect(humanizeNumber(999999)).toBe('1000K');
    });

    it('should add M suffix for millions', () => {
      expect(humanizeNumber(1000000)).toBe('1M');
      expect(humanizeNumber(2500000)).toBe('2.5M');
    });

    it('should add B suffix for billions', () => {
      expect(humanizeNumber(1000000000)).toBe('1B');
    });
  });

  describe('humanizeDuration', () => {
    it('should return ms for very short durations', () => {
      expect(humanizeDuration(100)).toBe('100ms');
      expect(humanizeDuration(999)).toBe('999ms');
    });

    it('should format seconds', () => {
      expect(humanizeDuration(1000)).toBe('1s');
      expect(humanizeDuration(1500)).toBe('1s500ms');
      expect(humanizeDuration(59000)).toBe('59s');
    });

    it('should format minutes', () => {
      expect(humanizeDuration(60000)).toBe('1m');
      expect(humanizeDuration(90000)).toBe('1m 30s');
    });

    it('should format hours', () => {
      expect(humanizeDuration(3600000)).toBe('1h');
      expect(humanizeDuration(3661000)).toBe('1h 1m 1s');
    });

    it('should format days', () => {
      expect(humanizeDuration(86400000)).toBe('1d');
      expect(humanizeDuration(90061000)).toBe('1d 1h 1m 1s');
    });

    it('should handle zero', () => {
      expect(humanizeDuration(0)).toBe('0ms');
    });
  });

  describe('debounceAsyncFunction', () => {
    function deferredRuns() {
      const runs: { arg: number; finish: () => void }[] = [];
      const fn = debounceAsyncFunction((arg: number) =>
        new Promise<number>((resolve) => runs.push({ arg, finish: () => resolve(arg) }))
      );
      return { fn, runs };
    }

    it('runs the calls that came during a run once, with the last arguments', async () => {
      const { fn, runs } = deferredRuns();
      const first = fn(1);
      const second = fn(2);
      const third = fn(3);
      expect(runs.map((r) => r.arg)).toEqual([1]);

      runs[0].finish();
      await expect(first).resolves.toBe(1);
      await vi.waitFor(() => expect(runs.map((r) => r.arg)).toEqual([1, 3]));
      runs[1].finish();
      await expect(second).resolves.toBe(3);
      await expect(third).resolves.toBe(3);
    });

    it('never runs twice at once', async () => {
      const { fn, runs } = deferredRuns();
      void fn(1);
      void fn(2);
      runs[0].finish();
      await vi.waitFor(() => expect(runs).toHaveLength(2));
      void fn(3);
      expect(runs).toHaveLength(2);
      runs[1].finish();
      await vi.waitFor(() => expect(runs.map((r) => r.arg)).toEqual([1, 2, 3]));
    });

    it('settles the callers of a run that failed', async () => {
      const fn = debounceAsyncFunction((arg: number) =>
        arg === 1 ? Promise.reject(new Error('one')) : Promise.resolve(arg)
      );
      const first = fn(1);
      const second = fn(2);
      await expect(first).rejects.toThrow('one');
      await expect(second).resolves.toBe(2);
    });
  });
});
