import type { Graph } from '@nodarium/types';

/** A drag holds its builds back when the nodes it changes took at least this long to run */
export const HOLD_OVER_MS = 250;
/** A held value is built once it rests for this long with the pointer still down */
export const REST_MS = 350;
/**
 * A held value is built this long after the release. The last value of a drag arrives after
 * the release: the effect that sends it runs in a microtask and the result is throttled.
 */
export const SETTLE_MS = 40;

function nodeKey(node: Graph['nodes'][number]) {
  return JSON.stringify([node.type, node.props, node.muted]);
}

function groupKey(group: Graph['groups'][number]) {
  return JSON.stringify([group.nodes.map(nodeKey), group.edges, group.inputs, group.outputs]);
}

/** The ids of the nodes of `next` that run with other values or inputs than in `base` */
export function changedNodes(base: Graph, next: Graph): Set<number> {
  const changed = new Set<number>();

  const groups = new Map((base.groups ?? []).map((g) => [g.id, groupKey(g)]));
  const changedGroups = new Set(
    (next.groups ?? []).filter((g) => groups.get(g.id) !== groupKey(g)).map((g) => g.id)
  );
  // a group that contains a changed group has changed too
  for (let grown = true; grown;) {
    grown = false;
    for (const group of next.groups ?? []) {
      if (changedGroups.has(group.id)) continue;
      if (group.nodes.some((n) => changedGroups.has(n.props?.groupId as number))) {
        changedGroups.add(group.id);
        grown = true;
      }
    }
  }

  const nodes = new Map(base.nodes.map((n) => [n.id, nodeKey(n)]));
  for (const node of next.nodes) {
    if (nodes.get(node.id) !== nodeKey(node)) changed.add(node.id);
    if (changedGroups.has(node.props?.groupId as number)) changed.add(node.id);
  }

  const edges = new Set(base.edges.map((e) => e.join()));
  const nextEdges = new Set(next.edges.map((e) => e.join()));
  for (const edge of next.edges) if (!edges.has(edge.join())) changed.add(edge[2]);
  for (const edge of base.edges) if (!nextEdges.has(edge.join())) changed.add(edge[2]);

  return changed;
}

/**
 * How long `changed` and every node downstream of them took the last time they ran, what a
 * change costs that was never built before
 */
export function downstreamCost(
  changed: Iterable<number>,
  graph: Graph,
  timings: Record<number, number>
): number {
  const targets = new Map<number, number[]>();
  for (const [from, , to] of graph.edges) {
    const list = targets.get(from);
    if (list) list.push(to);
    else targets.set(from, [to]);
  }

  const reached = new Set<number>();
  const queue = [...changed];
  let cost = 0;
  while (queue.length) {
    const id = queue.pop()!;
    if (reached.has(id)) continue;
    reached.add(id);
    cost += timings[id] ?? 0;
    queue.push(...(targets.get(id) ?? []));
  }
  return cost;
}

type DragHoldOptions = {
  build: (graph: Graph) => void;
  /** The time of every node the last time it ran, group nodes with the sum of their nodes */
  timings: () => Record<number, number>;
  holdOverMs?: number;
  restMs?: number;
  settleMs?: number;
};

/**
 * Keeps a drag from starting builds of values that are gone by the time they finish. While a
 * pointer is dragged, a change that rebuilds slow nodes waits until the value rests or the
 * pointer is released. Cheap changes, and changes without a drag (typing, steppers, keys),
 * are built at once.
 *
 * A change costs what its last two builds took at least. A change that wasn't built yet costs the time of
 * its nodes and everything downstream, which is too much when only a part of a group depends
 * on the changed input.
 */
export function createDragHold({
  build,
  timings,
  holdOverMs = HOLD_OVER_MS,
  restMs = REST_MS,
  settleMs = SETTLE_MS
}: DragHoldOptions) {
  let built: Graph | undefined;
  let held: Graph | undefined;
  let pressed = false;
  let dragging = false;
  let settling = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  // the last two build times, keyed by the ids of the changed nodes
  const learned = new Map<string, [number, number]>();
  const changes = new WeakMap<Graph, number[]>();

  function changesOf(graph: Graph) {
    let changed = changes.get(graph);
    if (!changed) {
      changed = built?.id === graph.id
        ? [...changedNodes(built, graph)].sort((a, b) => a - b)
        : [];
      changes.set(graph, changed);
    }
    return changed;
  }

  function cost(graph: Graph) {
    const changed = changesOf(graph);
    if (!changed.length) return 0;
    // one slow build, e.g. one that had to set up its wasm instances again, doesn't hold a drag
    const times = learned.get(changed.join());
    return times ? Math.min(...times) : downstreamCost(changed, graph, timings());
  }

  function run(graph: Graph) {
    changesOf(graph);
    if (built?.id !== graph.id) learned.clear();
    clearTimeout(timer);
    held = undefined;
    settling = false;
    built = graph;
    build(graph);
  }

  function flush() {
    settling = false;
    if (held) run(held);
  }

  function request(graph: Graph) {
    if (settling) {
      held = graph;
      return;
    }
    if (!dragging || cost(graph) < holdOverMs) {
      run(graph);
      return;
    }
    held = graph;
    clearTimeout(timer);
    timer = setTimeout(flush, restMs);
  }

  /**
   * Remembers what the build of `graph` took, `runTimings` are the times of the nodes that ran.
   * A build whose changed nodes came from the cache says nothing about the next value.
   */
  function measured(graph: Graph, runTimings: Record<number, number>) {
    const changed = changes.get(graph);
    if (!changed?.length || !changed.every((id) => id in runTimings)) return;
    // the nodes inside a group are in there too, their group node has their sum
    const time = graph.nodes.reduce((sum, n) => sum + (runTimings[n.id] ?? 0), 0);
    const key = changed.join();
    learned.set(key, [learned.get(key)?.[1] ?? time, time]);
  }

  function press() {
    pressed = true;
    dragging = false;
  }

  function move(buttons: number) {
    if (!pressed) return;
    if (buttons === 0) release();
    else dragging = true;
  }

  function release() {
    if (!pressed) return;
    pressed = false;
    dragging = false;
    if (!held) return;
    settling = true;
    clearTimeout(timer);
    timer = setTimeout(flush, settleMs);
  }

  /** Follows the pointer on `target`, in the capture phase so no handler can hide it */
  function listen(target: EventTarget) {
    const onMove = (e: Event) => move((e as MouseEvent).buttons ?? 0);
    // focus events of elements reach the window in the capture phase too
    const onBlur = (e: Event) => e.target === target && release();
    const listeners: [string, (e: Event) => void][] = [
      ['pointerdown', press],
      ['mousedown', press],
      ['pointermove', onMove],
      ['mousemove', onMove],
      ['pointerup', release],
      ['mouseup', release],
      ['pointercancel', release],
      ['blur', onBlur]
    ];
    const controller = new AbortController();
    for (const [type, fn] of listeners) {
      target.addEventListener(type, fn, { capture: true, signal: controller.signal });
    }
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }

  return { request, measured, press, move, release, listen };
}
