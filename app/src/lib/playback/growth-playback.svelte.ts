/** ms the growth takes when every build is fast enough */
const DURATION = 7000;
/** A slow build moves the time on by this at most: the growth gets slower, not choppy */
const MAX_STEP = 1 / 60;
/** ms after which very slow builds do get choppy */
const MAX_DURATION = 20000;

/** How far the growth moves on after a step that took `ms` */
export function growthStep(ms: number): number {
  return Math.min(ms / DURATION, Math.max(MAX_STEP, ms / MAX_DURATION));
}

export function frameBucket(ms: number) {
  if (ms < 34) return '<34ms';
  if (ms < 100) return '34-100ms';
  if (ms < 300) return '100-300ms';
  return '>300ms';
}

type Options = {
  /** Builds and shows the plant at a share of its growth, 0 to 1 */
  show: (share: number) => Promise<void>;
  /** Shows the graph as it is again */
  restore: () => Promise<void>;
  onerror?: (error: unknown) => void;
  /** The plant is grown, `frameMs` is the mean time of a step */
  onfinish?: (frameMs: number) => void;
  nextFrame?: () => Promise<number>;
};

const animationFrame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

/** Plays the growth of a plant, one build after the other */
export class GrowthPlayback {
  #status = $state<'idle' | 'playing' | 'paused'>('idle');
  #share = 0;
  /** Bumped when a run ends, a loop that waits for its build sees that it is not wanted anymore */
  #run = 0;
  #building: Promise<void> | undefined;
  #playedMs = 0;
  #steps = 0;

  constructor(private options: Options) {}

  get status() {
    return this.#status;
  }

  play() {
    if (this.#status === 'playing') return;
    if (this.#status === 'idle') {
      this.#share = 0;
      this.#playedMs = 0;
      this.#steps = 0;
    }
    this.#status = 'playing';
    void this.#loop(++this.#run);
  }

  pause() {
    if (this.#status !== 'playing') return;
    this.#run++;
    this.#status = 'paused';
  }

  /** Ends the growth and shows the graph as it is */
  async stop() {
    if (await this.cancel()) await this.#restore();
  }

  /** Ends the growth, what is shown after it is up to the caller. False if none was played. */
  async cancel(): Promise<boolean> {
    const active = this.#status !== 'idle';
    this.#run++;
    this.#status = 'idle';
    // also that of a growth that just ended, it would be shown after what the caller builds
    await this.#building;
    return active;
  }

  async #loop(run: number) {
    const { show, onerror, onfinish, nextFrame = animationFrame } = this.options;
    await this.#building;
    let last = await nextFrame();
    while (run === this.#run) {
      this.#building = show(this.#share).catch((error) => {
        if (run !== this.#run) return;
        this.#run++;
        this.#status = 'idle';
        onerror?.(error);
      });
      await this.#building;
      if (run !== this.#run) return;
      if (this.#share >= 1) {
        this.#status = 'idle';
        onfinish?.(this.#playedMs / this.#steps);
        await this.#restore();
        return;
      }
      const now = await nextFrame();
      if (run !== this.#run) return;
      this.#playedMs += now - last;
      this.#steps++;
      this.#share = Math.min(1, this.#share + growthStep(now - last));
      last = now;
    }
  }

  async #restore() {
    const { restore, onerror } = this.options;
    this.#building = restore().catch((error) => onerror?.(error));
    await this.#building;
  }
}
