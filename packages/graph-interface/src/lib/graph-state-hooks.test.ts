import { describe, expect, it } from 'vitest';
import { GraphManager } from './graph-manager.svelte';
import { GraphState } from './graph-state.svelte';
import { createGraphOptions, createMockNodeRegistry } from './test-utils';

const at = {
  nodeId: 3,
  nodeType: 'me/vj/blur',
  key: 'amount',
  input: { type: 'float' },
  x: 1,
  y: 2
} as const;

function createState() {
  const options = createGraphOptions();
  return new GraphState(new GraphManager(createMockNodeRegistry([])), () => options);
}

describe('the hooks of the app on an input', () => {
  it('draw no badge without the prop, and none for an empty text', () => {
    const state = createState();
    expect(state.inputBadge(3, 'amount')).toBeUndefined();
    state.readInputBadge = () => (_, key) => key === 'amount' ? 'kick' : '';
    expect(state.inputBadge(3, 'amount')).toBe('kick');
    expect(state.inputBadge(3, 'size')).toBeUndefined();
  });

  it('leave a right click to the editor unless the app prevents its default', () => {
    const state = createState();
    expect(state.openInputContextMenu(at)).toBe(false);

    state.readInputContextMenu = () => () => {};
    expect(state.openInputContextMenu(at)).toBe(false);

    state.readInputContextMenu = () => (event) => event.preventDefault();
    expect(state.openInputContextMenu(at)).toBe(true);
  });
});
