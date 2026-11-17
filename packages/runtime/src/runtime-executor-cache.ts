import { type SyncCache } from '@nodarium/types';

/** The bytes of the arrays a value holds, other values count as nothing */
export function byteSize(value: unknown): number {
  if (ArrayBuffer.isView(value)) return value.byteLength;
  if (Array.isArray(value)) return value.reduce((sum: number, v) => sum + byteSize(v), 0);
  return 0;
}

const MB = 2 ** 20;

/** Values up to this size are always kept, nearly every result of a plant */
export const ALWAYS_KEEP_BYTES = MB;
/**
 * A bigger value is only kept when it took at least this long per MB to make. The meshes at the
 * end of a mushroom take less, they pass a whole mesh on: a hit would save 7–27 ms for 50–90 MB,
 * and copying them into the cache takes a good part of that on every build.
 */
export const MIN_MS_PER_MB = 1;
/**
 * One time of a node that does little per byte can be up to this much too slow: its first run
 * sets up its module, and a run that a collection or another process interrupts takes about twice
 * as long. The skirt of the fly agaric makes 1.2 MB in 0.9–1.0 ms warm, in 1.1–1.3 ms on its
 * first run and now and then in 1.9 ms. Kept by one time, it stayed out of the cache in some
 * processes and got in in others, by the first run that happened to be slow.
 */
const TIME_SCATTER = 2;
/** Times below this are noise, a page without cross-origin isolation measures in steps of it */
const MIN_COST = 0.1;

type Entry = { value: unknown; bytes: number; cost: number; run: number };

/**
 * Keeps values by the time they save per byte, under a byte budget.
 *
 * - A value over {@link ALWAYS_KEEP_BYTES} that took less than {@link MIN_MS_PER_MB} isn't
 *   kept, {@link keeps} tells the caller before it copies one. A value it turned away is
 *   measured again when it is made again, the least of its times decides. One time decides only
 *   when it is at least twice the limit, a time can be that much too slow.
 * - The values read or written since the last {@link startRun} are the current run, the next
 *   change starts from it. They are removed last.
 * - Of the older values, the one that saves the least time per byte, divided by the runs since
 *   it was used, goes first. A value that saves seconds outlives many cheap runs, one that is
 *   never read again goes in the end. Equal values go in the order they were used.
 * - Over {@link size} values, the least recently used are removed, older runs first.
 */
export class MemoryRuntimeCache implements SyncCache {
  private map = new Map<string, Entry>();
  // the least time of the values turned away, so a slower time later doesn't let one in
  private turnedAway = new Map<string, number>();
  private totalBytes = 0;
  private maxEntries: number;
  private byteLimit: number;
  private run = 0;

  constructor(size = 50, maxBytes = Infinity) {
    this.maxEntries = size;
    this.byteLimit = maxBytes;
  }

  get size() {
    return this.maxEntries;
  }

  set size(size: number) {
    this.maxEntries = size;
    this.evict();
  }

  /** When the values hold more bytes than this, those worth the least are removed */
  get maxBytes() {
    return this.byteLimit;
  }

  set maxBytes(bytes: number) {
    this.byteLimit = bytes;
    this.evict();
  }

  /** The bytes of all values in the cache */
  get bytes() {
    return this.totalBytes;
  }

  /**
   * Whether {@link set} stores a value of `bytes` that took `cost` ms to make. With the `key` of
   * the value, a value it turns away counts as measured, the next time the least of its times
   * decides.
   */
  keeps(bytes: number, cost: number, key?: string) {
    if (bytes <= ALWAYS_KEEP_BYTES) return true;
    const earlier = key === undefined ? undefined : this.turnedAway.get(key);
    const least = Math.min(cost, earlier ?? Infinity);
    const needed = MIN_MS_PER_MB * bytes / MB * (earlier === undefined ? TIME_SCATTER : 1);
    if (least >= needed) return true;
    if (key !== undefined) this.turnAway(key, least);
    return false;
  }

  private turnAway(key: string, cost: number) {
    this.turnedAway.delete(key);
    this.turnedAway.set(key, cost);
    if (this.turnedAway.size > this.maxEntries) {
      this.turnedAway.delete(this.turnedAway.keys().next().value!);
    }
  }

  startRun() {
    this.run++;
  }

  get<T>(key: string): T | undefined {
    const entry = this.map.get(key);
    if (!entry) return undefined;
    this.map.delete(key);
    this.map.set(key, entry);
    entry.run = this.run;
    return entry.value as T;
  }

  /**
   * @param cost the time it took to make the value in ms, a value without one is stored
   * whatever its size
   */
  set<T>(key: string, value: T, cost?: number): void {
    this.delete(key);
    const bytes = byteSize(value);
    if (cost !== undefined && !this.keeps(bytes, cost, key)) return;
    cost = Math.min(cost ?? 0, this.turnedAway.get(key) ?? Infinity);
    this.turnedAway.delete(key);
    this.map.set(key, { value, bytes, cost, run: this.run });
    this.totalBytes += bytes;
    this.evict();
  }

  clear(): void {
    this.map.clear();
    this.turnedAway.clear();
    this.totalBytes = 0;
  }

  private delete(key: string) {
    const entry = this.map.get(key);
    if (!entry) return;
    this.map.delete(key);
    this.totalBytes -= entry.bytes;
  }

  private evict() {
    while (this.map.size > this.maxEntries) {
      this.delete(this.lowest((entry) => entry.run - this.run));
    }
    while (this.totalBytes > this.byteLimit && this.map.size) {
      this.delete(this.lowest((entry) => this.worth(entry)));
    }
  }

  // ms saved per MB, shared by the runs since it was used
  private worth(entry: Entry) {
    const value = Math.max(entry.cost, MIN_COST) * MB / Math.max(entry.bytes, 1);
    return value / Math.max(this.run - entry.run, 1);
  }

  /** The key of the entry with the lowest rank, older runs before the current one */
  private lowest(rank: (entry: Entry) => number) {
    let lowest: string | undefined;
    let lowestRank = Infinity;
    let lowestCurrent = true;
    // the map is in the order of use, so equal ranks go to the least recently used
    for (const [key, entry] of this.map) {
      const current = entry.run === this.run;
      if (current && !lowestCurrent) continue;
      const r = rank(entry);
      if (lowest === undefined || (!current && lowestCurrent) || r < lowestRank) {
        lowest = key;
        lowestRank = r;
        lowestCurrent = current;
      }
    }
    return lowest!;
  }
}
