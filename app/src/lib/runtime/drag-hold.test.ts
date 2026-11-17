import type { Graph, NodeInput } from '@nodarium/types';
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

const inputs: Record<string, Record<string, NodeInput>> = {
  'test/node/output': {
    a: { type: 'float' },
    b: { type: 'float' },
    circle: { type: 'integer', setting: 'resolution.circle' }
  },
  'test/node/mesh': {
    length: { type: 'float' },
    curve: { type: 'integer', setting: 'resolution.curve' }
  },
  'test/node/random': { max: { type: 'float' }, seed: { type: 'seed' } },
  'test/node/time': { t: { type: 'float', setting: 'time.t' } }
};
const inputsOf = (type: string) => inputs[type];

function withSettings(settings: Record<string, unknown>, extra: Partial<Graph> = {}) {
  return graph(1, 1, { settings, ...extra });
}

// the graph with 4 group instance -> 1 mesh, the instance holds a group that holds the time node
function withTimeInGroup(t: number, muted = false) {
  const instance = (id: number, groupId: number) => ({
    id,
    type: '__internal/group/instance',
    position: [0, 0],
    props: { groupId }
  });
  return withSettings({ 'time.t': t }, {
    nodes: [...graph().nodes, instance(4, 8)],
    edges: [...graph().edges, [4, 0, 1, 'curve']],
    groups: [
      { id: 7, nodes: [{ id: 0, type: 'test/node/time', position: [0, 0], muted }], edges: [] },
      { id: 8, nodes: [instance(0, 7)], edges: [] }
    ]
  } as Partial<Graph>);
}

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

describe('changedNodes of a changed setting', () => {
  const changed = (base: Graph, next: Graph) =>
    [...changedNodes(base, next, inputsOf)].sort((a, b) => a - b);

  it('finds the nodes with an input that reads the setting', () => {
    const base = withSettings({ 'resolution.curve': 10, 'resolution.circle': 8 });
    expect(changed(base, withSettings({ 'resolution.curve': 20, 'resolution.circle': 8 })))
      .toEqual([1]);
    expect(changed(base, withSettings({ 'resolution.curve': 10, 'resolution.circle': 16 })))
      .toEqual([0]);
    expect(changed(base, withSettings({ 'resolution.curve': 10, 'resolution.circle': 8 })))
      .toEqual([]);
  });

  it('finds nothing for a setting no node reads', () => {
    expect(changed(withSettings({ 'time.t': 0.1 }), withSettings({ 'time.t': 0.2 }))).toEqual([]);
  });

  it('finds the nodes with a seed input for the seed, no node declares it as a setting', () => {
    expect(changed(withSettings({ seed: 1 }), withSettings({ seed: 2 }))).toEqual([2, 3]);
  });

  it('compares the values of a setting, not their objects', () => {
    const setting = () => withSettings({ 'resolution.curve': [1, 2, 3] });
    expect(changed(setting(), setting())).toEqual([]);
  });

  it('leaves out a muted node, it does not run', () => {
    const next = withSettings({ seed: 2 });
    next.nodes[2].muted = true;
    const base = withSettings({ seed: 1 });
    base.nodes[2].muted = true;
    expect(changed(base, next)).toEqual([3]);
  });

  it('counts the group nodes of a group whose nodes read the setting', () => {
    expect(changed(withTimeInGroup(0.1), withTimeInGroup(0.2))).toEqual([4]);
    expect(changed(withTimeInGroup(0.1, true), withTimeInGroup(0.2, true))).toEqual([]);
  });

  it('knows no setting without the inputs of the nodes', () => {
    expect([...changedNodes(withSettings({ seed: 1 }), withSettings({ seed: 2 }))]).toEqual([]);
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

  it('says that it holds a change until the change is built', () => {
    expect(hold.holding).toBe(false);
    drag([2]);
    expect(hold.holding).toBe(true);
    hold.release();
    vi.advanceTimersByTime(SETTLE_MS);
    expect(hold.holding).toBe(false);
    expect(builds).toHaveLength(2);
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

describe('createDragHold with a dragged setting', () => {
  let builds: Graph[];
  let hold: ReturnType<typeof createDragHold>;

  beforeEach(() => {
    vi.useFakeTimers();
    builds = [];
    hold = createDragHold({
      build: (g) => builds.push(g),
      timings: () => ({ ...timings, 4: 1 }),
      inputs: inputsOf
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function drag(key: string, values: number[], change = (v: number) => withSettings({ [key]: v })) {
    hold.request(change(0));
    hold.press();
    for (const v of values) {
      hold.move(1);
      hold.request(change(v));
    }
  }

  it('holds a setting that a slow node reads and builds the last value after the release', () => {
    drag('resolution.curve', [1, 2, 3]);
    hold.release();
    expect(builds).toHaveLength(1);

    vi.advanceTimersByTime(SETTLE_MS);
    expect(builds.map((g) => g.settings!['resolution.curve'])).toEqual([0, 3]);
  });

  it('builds the value of a held setting that rests while the pointer is still down', () => {
    drag('resolution.curve', [1, 2]);
    vi.advanceTimersByTime(REST_MS);
    expect(builds.map((g) => g.settings!['resolution.curve'])).toEqual([0, 2]);
  });

  it('holds a setting whose nodes are cheap but feed a slow node', () => {
    drag('seed', [1, 2, 3]);
    expect(builds).toHaveLength(1);
  });

  it('holds a setting that a node inside a group reads', () => {
    drag('time.t', [0.1, 0.2], withTimeInGroup);
    expect(builds).toHaveLength(1);
  });

  it('keeps a setting live that only cheap nodes read', () => {
    drag('resolution.circle', [1, 2, 3]);
    expect(builds).toHaveLength(4);
  });

  it('keeps a setting live that no node reads', () => {
    drag('time.t', [0.1, 0.2, 0.3]);
    expect(builds).toHaveLength(4);
  });

  it('keeps a setting live once a build showed that the change is cheap', () => {
    drag('resolution.curve', []);
    hold.request(withSettings({ 'resolution.curve': 1 }));
    hold.measured(builds.at(-1)!, { 0: 5, 1: 100 });
    hold.release();
    drag('resolution.curve', [2, 3]);
    expect(builds).toHaveLength(5);
  });
});
