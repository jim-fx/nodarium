import { describe, expect, it, vi } from 'vitest';
import { createKeyMap, isTextEntry } from './keymap';

function press(
  key: string,
  init: Partial<Omit<KeyboardEvent, 'target'>> & { target?: object } = {}
) {
  return {
    key,
    shiftKey: false,
    ctrlKey: false,
    altKey: false,
    target: { tagName: 'DIV', isContentEditable: false },
    preventDefault: () => {},
    ...init
  } as unknown as KeyboardEvent;
}

function helpKeymap() {
  const callback = vi.fn();
  const keymap = createKeyMap([{ key: '?', ctrl: false, alt: false, callback }]);
  return { keymap, callback };
}

describe('createKeyMap', () => {
  it('takes ? with the shift it needs on a US and on a German keyboard', () => {
    const { keymap, callback } = helpKeymap();
    // Shift+/ and Shift+ß both give the key `?`
    keymap.handleKeyboardEvent(press('?', { shiftKey: true }));
    expect(callback).toHaveBeenCalledOnce();
  });

  it('leaves ? alone with Ctrl or Alt held', () => {
    const { keymap, callback } = helpKeymap();
    keymap.handleKeyboardEvent(press('?', { shiftKey: true, ctrlKey: true }));
    keymap.handleKeyboardEvent(press('?', { shiftKey: true, altKey: true }));
    expect(callback).not.toHaveBeenCalled();
  });

  it('leaves the shift key and ß alone', () => {
    const { keymap, callback } = helpKeymap();
    keymap.handleKeyboardEvent(press('Shift', { shiftKey: true }));
    keymap.handleKeyboardEvent(press('ß'));
    keymap.handleKeyboardEvent(press('/'));
    expect(callback).not.toHaveBeenCalled();
  });

  it('fires no shortcut while typing in an input, a textarea, a select or editable text', () => {
    const { keymap, callback } = helpKeymap();
    for (const tagName of ['INPUT', 'TEXTAREA', 'SELECT']) {
      keymap.handleKeyboardEvent(
        press('?', { shiftKey: true, target: { tagName } })
      );
    }
    keymap.handleKeyboardEvent(
      press('?', {
        shiftKey: true,
        target: { tagName: 'DIV', isContentEditable: true }
      })
    );
    expect(callback).not.toHaveBeenCalled();
  });
});

describe('isTextEntry', () => {
  it('knows the elements that take text', () => {
    expect(isTextEntry({ tagName: 'INPUT' })).toBe(true);
    expect(isTextEntry({ tagName: 'SPAN', isContentEditable: true })).toBe(true);
    expect(isTextEntry({ tagName: 'BUTTON', isContentEditable: false })).toBe(false);
    expect(isTextEntry(null)).toBe(false);
  });
});
