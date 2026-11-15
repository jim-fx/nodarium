import type { AsyncCache } from '@nodarium/types';
import { type IDBPDatabase, openDB } from 'idb';

export class IndexDBCache implements AsyncCache<unknown> {
  size: number = 100;

  db: Promise<IDBPDatabase<unknown>>;
  private _cache = new Map<string, unknown>();

  constructor(id: string) {
    this.db = openDB<unknown>('cache/' + id, 1, {
      upgrade(db) {
        db.createObjectStore('keyval');
      }
    });
  }

  async get<T>(key: string): Promise<T> {
    let res = this._cache.get(key);
    if (!res) {
      res = await (await this.db).get('keyval', key);
    }
    if (res) {
      this._cache.set(key, res);
    }
    return res as T;
  }

  async set(key: string, value: unknown) {
    this._cache.set(key, value);
    const db = await this.db;
    await db.put('keyval', value, key);
  }

  clear() {
    this.db.then(db => db.clear('keyval'));
  }
}
