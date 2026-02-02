import { type SyncCache } from '@nodarium/types';

export class MemoryRuntimeCache implements SyncCache {
  private map = new Map<string, unknown>();
  size: number;

  constructor(size = 50) {
    this.size = size;
  }

  get<T>(key: string): T | undefined {
    if (!this.map.has(key)) return undefined;
    const value = this.map.get(key) as T;
    this.map.delete(key);
    this.map.set(key, value);
    return value;
  }

  set<T>(key: string, value: T): void {
    if (this.map.has(key)) {
      this.map.delete(key);
    }
    this.map.set(key, value);
    while (this.map.size > this.size) {
      const oldestKey = this.map.keys().next().value as string;
      this.map.delete(oldestKey);
    }
  }

  clear(): void {
    this.map.clear();
  }
}
