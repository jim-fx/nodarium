import { derived, get, writable } from 'svelte/store';

export type ShortCut = {
  key: string | string[];
  shift?: boolean;
  ctrl?: boolean;
  alt?: boolean;
  preventDefault?: boolean;
  description?: string;
  callback: (event: KeyboardEvent) => void;
};

function getShortcutId(shortcut: ShortCut) {
  return `${shortcut.key}${shortcut.shift ? '+shift' : ''}${shortcut.ctrl ? '+ctrl' : ''}${
    shortcut.alt ? '+alt' : ''
  }`;
}

export function createKeyMap(keys: ShortCut[]) {
  const store = writable(new Map(keys.map(k => [getShortcutId(k), k])));

  return {
    handleKeyboardEvent: (event: KeyboardEvent) => {
      const activeElement = document.activeElement as HTMLElement;
      if (activeElement?.tagName === 'INPUT' || activeElement?.tagName === 'TEXTAREA') return;
      const key = [...get(store).values()].find(k => {
        if (Array.isArray(k.key) ? !k.key.includes(event.key) : k.key !== event.key) return false;
        if ('shift' in k && k.shift !== event.shiftKey) return false;
        if ('ctrl' in k && k.ctrl !== event.ctrlKey) return false;
        if ('alt' in k && k.alt !== event.altKey) return false;
        return true;
      });
      if (key && key.preventDefault) event.preventDefault();
      key?.callback(event);
    },
    addShortcut: (shortcut: ShortCut) => {
      if (Array.isArray(shortcut.key)) {
        for (const k of shortcut.key) {
          store.update(shortcuts => {
            const id = getShortcutId({ ...shortcut, key: k });
            shortcuts.delete(id);
            shortcuts.set(id, { ...shortcut, key: k });
            return shortcuts;
          });
        }
      } else {
        store.update(shortcuts => {
          const id = getShortcutId(shortcut);
          shortcuts.delete(id);
          shortcuts.set(id, shortcut);
          return shortcuts;
        });
      }
    },
    keys: derived(store, $store => Array.from($store.values()))
  };
}
