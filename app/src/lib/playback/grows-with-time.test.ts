import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { grownTime, growsWithTime } from './grows-with-time';

const graph = (value: object) => ({ id: 0, groups: [], ...value }) as unknown as Graph;

const nodes = [
  { id: 1, type: 'max/core/time' },
  { id: 2, type: 'max/plantarium/stem' },
  { id: 3, type: 'max/core/output' }
];

describe('growsWithTime', () => {
  it('needs a time node that the output depends on', () => {
    expect(growsWithTime(graph({ nodes, edges: [[1, 0, 2, 'length'], [2, 0, 3, 'input']] }))).toBe(
      true
    );
    expect(growsWithTime(graph({ nodes, edges: [[2, 0, 3, 'input']] }))).toBe(false);
    expect(growsWithTime(graph({ nodes, edges: [[1, 0, 2, 'length']] }))).toBe(false);
  });

  it('finds the time node of a group that is used', () => {
    const groups = [{ id: 7, nodes: nodes.slice(0, 2), edges: [[1, 0, 2, 'length']] }];
    const group = { id: 4, type: '__internal/group/instance', props: { groupId: 7 } };
    const all = [group, nodes[2]];
    expect(growsWithTime(graph({ nodes: all, edges: [[4, 0, 3, 'input']], groups }))).toBe(true);
    expect(growsWithTime(graph({ nodes: all, edges: [], groups }))).toBe(false);
  });
});

describe('grownTime', () => {
  it('is the time the graph is saved at', () => {
    expect(grownTime(graph({ settings: { 'time.t': 0.6 } }))).toBe(0.6);
    expect(grownTime(graph({ settings: { 'time.t': 0 } }))).toBe(1);
    expect(grownTime(graph({}))).toBe(1);
  });
});
