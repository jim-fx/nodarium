import { type SyncCache } from '@nodarium/types';

/** The bytes of the arrays a value holds, other values count as nothing */
export function byteSize(value: unknown): number {
  if (ArrayBuffer.isView(value)) return value.byteLength;
  if (Array.isArray(value)) return value.reduce((sum: number, v) => sum + byteSize(v), 0);
  return 0;
}

export class MemoryRuntimeCache implements SyncCache {
  private map = new Map<string, { value: unknown; bytes: number }>();
  private totalBytes = 0;
  private maxEntries: number;
  private byteLimit: number;

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

  /** When the values hold more bytes than this, the least recently used are removed */
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

  get<T>(key: string): T | undefined {
    const entry = this.map.get(key);
    if (!entry) return undefined;
    this.map.delete(key);
    this.map.set(key, entry);
    return entry.value as T;
  }

  set<T>(key: string, value: T): void {
    this.delete(key);
    const bytes = byteSize(value);
    this.map.set(key, { value, bytes });
    this.totalBytes += bytes;
    this.evict();
  }

  clear(): void {
    this.map.clear();
    this.totalBytes = 0;
  }

  private delete(key: string) {
    const entry = this.map.get(key);
    if (!entry) return;
    this.map.delete(key);
    this.totalBytes -= entry.bytes;
  }

  /** Removes the least recently used values until the cache fits its limits */
  private evict() {
    for (const key of this.map.keys()) {
      if (this.map.size <= this.maxEntries && this.totalBytes <= this.byteLimit) return;
      this.delete(key);
    }
  }
}
