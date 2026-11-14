import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { HistoryManager } from './history-manager.ts';

const graph = (value: number) =>
  ({ id: 1, meta: { value }, nodes: [], edges: [] }) as unknown as Graph;
const valueOf = (g: Graph | undefined) => (g as unknown as { meta: { value: number } }).meta.value;

function fill(manager: HistoryManager, steps: number) {
  for (let i = 0; i <= steps; i++) manager.save(graph(i));
}

describe('HistoryManager', () => {
  it('undoes and redoes within the limit', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    fill(manager, 5);
    expect(valueOf(manager.undo())).toBe(4);
    expect(valueOf(manager.undo())).toBe(3);
    expect(valueOf(manager.redo())).toBe(4);
  });

  it('does nothing when redoing at the newest state', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    manager.save({ id: 1, meta: {}, nodes: [], edges: [] } as unknown as Graph);
    manager.save({ id: 1, meta: {}, nodes: [{ id: 1 }], edges: [] } as unknown as Graph);
    const index = manager.index;
    expect(manager.redo()).toBeUndefined();
    expect(manager.index).toBe(index);
    expect(manager.undo()).toEqual({ id: 1, meta: {}, nodes: [], edges: [] });
    expect(manager.redo()).toEqual({ id: 1, meta: {}, nodes: [{ id: 1 }], edges: [] });
  });

  it('keeps index inside the history after exceeding maxHistory', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    fill(manager, 35);
    expect(manager.history.length).toBe(10);
    expect(manager.index).toBe(9);
  });

  it('undoes all the way to the oldest covered state', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    fill(manager, 35);
    for (let expected = 34; expected >= 25; expected--) {
      expect(valueOf(manager.undo())).toBe(expected);
    }
    expect(manager.index).toBe(-1);
    expect(valueOf(manager.undo())).toBe(25);
  });

  it('redoes all the way after undoing everything', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    fill(manager, 35);
    for (let i = 0; i < 11; i++) manager.undo();
    for (let expected = 26; expected <= 35; expected++) {
      expect(valueOf(manager.redo())).toBe(expected);
    }
    expect(manager.index).toBe(9);
  });

  it('saves after an undo in the middle once the limit was exceeded', () => {
    const manager = new HistoryManager({ maxHistory: 10 });
    fill(manager, 35);
    for (let i = 0; i < 4; i++) manager.undo();
    manager.save(graph(100));
    expect(manager.history.length).toBe(7);
    expect(valueOf(manager.undo())).toBe(31);
    expect(valueOf(manager.redo())).toBe(100);
    for (let i = 0; i < 7; i++) manager.undo();
    expect(manager.index).toBe(-1);
    expect(valueOf(manager.undo())).toBe(25);
  });
});
