import { describe, expect, it, vi } from 'vitest';
import { GrowthPlayback, growthStep } from './growth-playback.svelte';

const until = (check: () => boolean) =>
  vi.waitFor(() => {
    if (!check()) throw new Error('not yet');
  }, { interval: 1 });

/** Frames of `ms`, and builds that wait until they are let through when `manual` is set */
function setup(ms = 500, manual = false) {
  let now = 0;
  const shown: number[] = [];
  const waiting: (() => void)[] = [];
  const restore = vi.fn(async () => {});
  const onfinish = vi.fn();
  const playback = new GrowthPlayback({
    show: async (share) => {
      if (manual) await new Promise<void>((resolve) => waiting.push(resolve));
      shown.push(share);
    },
    restore,
    onfinish,
    nextFrame: async () => {
      await new Promise((resolve) => setTimeout(resolve));
      return now += ms;
    }
  });
  return { playback, shown, waiting, restore, onfinish };
}

describe('growthStep', () => {
  it('follows the clock, slows down for slow builds and has a limit for very slow ones', () => {
    expect(growthStep(16) * 7000).toBeCloseTo(16);
    expect(growthStep(200)).toBe(1 / 60);
    expect(growthStep(2000)).toBe(0.1);
  });
});

describe('GrowthPlayback', () => {
  it('grows from 0 to 1 and then shows the graph as it is, again when played again', async () => {
    const { playback, shown, restore, onfinish } = setup();
    for (const times of [1, 2]) {
      shown.length = 0;
      playback.play();
      await until(() => restore.mock.calls.length === times);
      expect(shown[0]).toBe(0);
      expect(shown.at(-1)).toBe(1);
      expect(shown).toEqual([...shown].sort((a, b) => a - b));
      expect(playback.status).toBe('idle');
    }
    expect(onfinish).toHaveBeenCalledWith(500);
  });

  it('goes on where it was paused', async () => {
    const { playback, shown } = setup(5);
    playback.play();
    await until(() => shown.length > 3);
    playback.pause();
    const paused = shown.at(-1)!;
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(shown.at(-1)).toBe(paused);
    expect(playback.status).toBe('paused');
    playback.play();
    await until(() => shown.at(-1)! > paused);
    await playback.cancel();
  });

  it('a cancel waits for the build that runs, and shows nothing after it', async () => {
    const { playback, shown, waiting, restore } = setup(16, true);
    playback.play();
    await until(() => waiting.length === 1);
    let cancelled = false;
    const cancel = playback.cancel().then((active) => {
      cancelled = true;
      return active;
    });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(cancelled).toBe(false);
    waiting[0]();
    expect(await cancel).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(shown).toEqual([0]);
    expect(restore).not.toHaveBeenCalled();
    expect(await playback.cancel()).toBe(false);
  });
});
