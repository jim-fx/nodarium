import { type Writable, writable } from 'svelte/store';

function isStore(v: unknown): v is Writable<unknown> {
  return v !== null && typeof v === 'object' && 'subscribe' in v && 'set' in v;
}

const storeIds: Map<string, ReturnType<typeof createLocalStore>> = new Map();

const HAS_LOCALSTORAGE = 'localStorage' in globalThis;

function createLocalStore<T>(key: string, initialValue: T | Writable<T>) {
  let store: Writable<T>;

  if (HAS_LOCALSTORAGE) {
    const localValue = localStorage.getItem(key);
    const value = localValue ? JSON.parse(localValue) : null;
    if (value === null) {
      if (isStore(initialValue)) {
        store = initialValue;
      } else {
        store = writable(initialValue);
      }
    } else {
      store = writable(value);
    }
  } else {
    return isStore(initialValue) ? initialValue : writable(initialValue);
  }

  store.subscribe((value) => {
    localStorage.setItem(key, JSON.stringify(value));
  });

  return {
    subscribe: store.subscribe,
    set: store.set,
    update: store.update
  };
}

export default function localStore<T>(key: string, initialValue: T | Writable<T>): Writable<T> {
  if (storeIds.has(key)) return storeIds.get(key) as Writable<T>;

  const store = createLocalStore(key, initialValue);

  storeIds.set(key, store);

  return store;
}
