import type { Graph } from '@nodarium/types';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { changedNodes, createDragHold, downstreamCost, REST_MS, SETTLE_MS } from './drag-hold';

// 0 output <- 1 mesh <- 2 early slider, 0 <- 3 late slider
function graph(early = 1, late = 1, extra: Partial<Graph> = {}): Graph {
  return {
    id: 0,
    nodes: [
      { id: 0, type: 'test/node/output', position: [0, 0] },
      { id: 1, type: 'test/node/mesh', position: [0, 0] },
      { id: 2, type: 'test/node/random', position: [0, 0], props: { max: early } },
      { id: 3, type: 'test/node/random', position: [0, 0], props: { max: late } }
    ],
    edges: [[1, 0, 0, 'a'], [2, 0, 1, 'length'], [3, 0, 0, 'b']],
    groups: [],
    ...extra
  } as Graph;
}

const timings = { 0: 5, 1: 2000, 2: 1, 3: 1 };

describe('downstreamCost', () => {
  it('adds up the changed node and everything downstream of it', () => {
    expect(downstreamCost([2], graph(), timings)).toBe(2006);
    expect(downstreamCost([3], graph(), timings)).toBe(6);
    expect(downstreamCost([], graph(), timings)).toBe(0);
  });
});

describe('changedNodes', () => {
  it('finds the nodes with other values', () => {
    expect([...changedNodes(graph(), graph(2))]).toEqual([2]);
    expect([...changedNodes(graph(), graph())]).toEqual([]);
  });

  it('ignores positions', () => {
    const moved = graph();
    moved.nodes[1].position = [10, 10];
    expect(changedNodes(graph(), moved).size).toBe(0);
  });

  it('counts the target of an edge that was added or removed', () => {
    const base = graph();
    const next = graph();
    next.edges = next.edges.filter((e) => e[0] !== 2);
    expect([...changedNodes(base, next)]).toEqual([1]);
    expect([...changedNodes(next, base)]).toEqual([1]);
  });

  it('counts the group nodes of a changed group, also through a group inside it', () => {
    const group = (id: number, nodes: Graph['nodes']) => ({ id, nodes, edges: [] });
    const inner = (max: number) =>
      group(7, [{ id: 0, type: 'test/node/random', position: [0, 0], props: { max } }]);
    const outer = group(8, [
      { id: 0, type: '__internal/group/instance', position: [0, 0], props: { groupId: 7 } }
    ]);
    const withGroups = (max: number) =>
      graph(1, 1, {
        nodes: [...graph().nodes, {
          id: 4,
          type: '__internal/group/instance',
          position: [0, 0],
          props: { groupId: 8 }
        }],
        groups: [inner(max), outer]
      });

    expect([...changedNodes(withGroups(1), withGroups(2))]).toEqual([4]);
  });
});

describe('createDragHold', () => {
  let builds: Graph[];
  let hold: ReturnType<typeof createDragHold>;

  beforeEach(() => {
    vi.useFakeTimers();
    builds = [];
    hold = createDragHold({ build: (g) => builds.push(g), timings: () => timings });
    hold.request(graph());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function drag(values: number[], change = (v: number) => graph(v)) {
    hold.press();
    for (const v of values) {
      hold.move(1);
      hold.request(change(v));
    }
  }

  it('builds a slow change at once without a drag', () => {
    hold.request(graph(2));
    expect(builds).toHaveLength(2);
  });

  it('builds a stepper at once, it changes the value before the pointer moves', () => {
    hold.press();
    hold.request(graph(2));
    hold.release();
    expect(builds).toHaveLength(2);
  });

  it('keeps a cheap drag live', () => {
    drag([2, 3, 4], (v) => graph(1, v));
    expect(builds.map((g) => g.nodes[3].props!.max)).toEqual([1, 2, 3, 4]);
  });

  it('holds a slow drag and builds the last value after the release', () => {
    drag([2, 3, 4]);
    hold.release();
    expect(builds).toHaveLength(1);

    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds).toHaveLength(2);
    expect(builds[1].nodes[2].props!.max).toBe(4);
  });

  it('takes the value that arrives just after the release', () => {
    drag([2, 3]);
    hold.release();
    hold.request(graph(4));
    vi.advanceTimersByTime(SETTLE_MS - 1);
    expect(builds).toHaveLength(1);

    vi.advanceTimersByTime(1);
    expect(builds.map((g) => g.nodes[2].props!.max)).toEqual([1, 4]);
  });

  it('builds the value that rests while the pointer is still down', () => {
    drag([2, 3]);
    vi.advanceTimersByTime(REST_MS);
    expect(builds.map((g) => g.nodes[2].props!.max)).toEqual([1, 3]);

    hold.move(1);
    hold.request(graph(4));
    hold.release();
    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds.map((g) => g.nodes[2].props!.max)).toEqual([1, 3, 4]);
  });

  it('builds nothing on a release when nothing was held', () => {
    drag([2, 3, 4], (v) => graph(1, v));
    hold.release();
    vi.advanceTimersByTime(REST_MS);
    expect(builds).toHaveLength(4);
  });

  it('takes a move without buttons as the release it missed', () => {
    drag([2]);
    hold.move(0);
    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds).toHaveLength(2);

    hold.request(graph(3));
    expect(builds).toHaveLength(3);
  });

  it('keeps a drag live once a build showed that the change is cheap', () => {
    hold.request(graph(1.5));
    hold.measured(builds.at(-1)!, { 0: 5, 1: 100, 2: 1 });
    drag([2, 3]);
    expect(builds).toHaveLength(4);
  });

  it('keeps a drag live after one slow build of a cheap change', () => {
    const slow = { 0: 5, 1: 2000, 2: 1 };
    hold.request(graph(1.5));
    hold.measured(builds.at(-1)!, { 0: 5, 1: 100, 2: 1 });
    hold.request(graph(1.6));
    hold.measured(builds.at(-1)!, slow);
    drag([2]);
    expect(builds).toHaveLength(4);

    hold.release();
    hold.measured(builds.at(-1)!, slow);
    drag([3]);
    expect(builds).toHaveLength(4);
  });

  it('forgets what it learned when another project is built', () => {
    hold.request(graph(1.5));
    hold.measured(builds.at(-1)!, { 0: 5, 1: 100, 2: 1 });
    hold.request(graph(1, 1, { id: 1 }));
    hold.request(graph(1.5, 1, { id: 1 }));
    drag([2], (v) => graph(v, 1, { id: 1 }));
    expect(builds).toHaveLength(4);
  });

  it('counts the nodes inside a group only once, through their group node', () => {
    hold.request(graph(1.5));
    hold.measured(builds.at(-1)!, { 0: 5, 1: 200, 2: 1, 12345: 150, 12346: 50 });
    drag([2, 3]);
    expect(builds).toHaveLength(4);
  });

  it('learns nothing from a build that read the changed node from the cache', () => {
    hold.request(graph(2));
    hold.measured(builds.at(-1)!, { 0: 5 });
    drag([3]);
    expect(builds).toHaveLength(2);
  });

  it('follows mouse and pointer events in the capture phase', () => {
    const target = new EventTarget();
    const stop = hold.listen(target);
    target.dispatchEvent(new Event('pointerdown'));
    target.dispatchEvent(Object.assign(new Event('mousemove'), { buttons: 1 }));
    hold.request(graph(2));
    expect(builds).toHaveLength(1);

    target.dispatchEvent(new Event('mouseup'));
    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds).toHaveLength(2);

    stop();
    hold.press();
    hold.move(1);
    hold.request(graph(3));
    target.dispatchEvent(new Event('mouseup'));
    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds).toHaveLength(2);
  });
});
