import { browser } from '$app/environment';

function mergeRecursive<T>(current: T, initial: T): T {
  if (typeof initial === 'number') {
    if (typeof current === 'number') return current;
    return initial;
  }

  if (typeof initial === 'boolean') {
    if (typeof current === 'boolean') return current;
    return initial;
  }

  if (Array.isArray(initial)) {
    if (Array.isArray(current)) return current;
    return initial;
  }

  if (typeof initial === 'object' && initial) {
    const merged = initial;
    if (typeof current === 'object' && current) {
      for (const key of Object.keys(initial)) {
        if (key in current) {
          // @ts-expect-error It's safe dont worry about it
          merged[key] = mergeRecursive(current[key], initial[key]);
        }
      }
    }
    return merged;
  }

  return current;
}

export class LocalStore<T> {
  value = $state<T>() as T;
  key = '';

  constructor(key: string, value: T) {
    this.key = key;
    this.value = value;

    if (browser) {
      const item = localStorage.getItem(key);
      if (item) {
        const storedValue = this.deserialize(item);
        this.value = mergeRecursive(storedValue, value);
      }
    }

    $effect.root(() => {
      $effect(() => {
        localStorage.setItem(this.key, this.serialize(this.value));
      });
    });
  }

  serialize(value: T): string {
    return JSON.stringify(value);
  }

  deserialize(item: string): T {
    return JSON.parse(item);
  }
}

export function localState<T>(key: string, value: T) {
  return new LocalStore(key, value);
}
