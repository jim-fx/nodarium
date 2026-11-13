import type { Graph, GroupDefinition, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, type Scope, scopes } from './helpers';

const CAP = 'max/myzel/cap';
const CAP_MARGIN = 'max/myzel/cap_margin';
const LAMELLAE = 'max/myzel/lamellae';
const FILL_GAPS = 'max/plantarium/fill_gaps';
const POINTS_ON_PATH = 'max/plantarium/points_on_path';
const SEPARATE = 'max/plantarium/separate';
const GROUP_INPUT = '__internal/group/input';
const GROUP_OUTPUT = '__internal/group/output';
const GROUP_INSTANCE = '__internal/group/instance';

/** The nodes that posed a cap from its rays and hinge and gave its margin as output 1 */
const POSE_NODES = ['cap_lopsided', 'cap_open', 'margin_curl', 'margin_waves'].map((name) =>
  `max/myzel/${name}`
);

const HINGE_OUTPUT = 1;
/** The output the margin was, on the cap and on the pose nodes */
const MARGIN_OUTPUT: Record<string, number> = {
  [CAP]: 2,
  ...Object.fromEntries(POSE_NODES.map((type) => [type, 1]))
};

/** The nodes the paths of a cap go through with their stipe, and the groups they leave by */
const TAKES_STIPE = new Set([
  FILL_GAPS,
  ...POSE_NODES,
  LAMELLAE,
  CAP_MARGIN,
  GROUP_OUTPUT,
  GROUP_INSTANCE
]);

/** Where the new nodes sit, from the node they belong to */
const BEFORE = [-15, 30];
const AFTER = [15, 25];

/**
 * Up to graph version 7 the cap grew at the tip of every stipe and gave only its rays, the hinge
 * (stipe top) and the margin. The pose nodes (margin_waves, cap_lopsided, margin_curl, cap_open)
 * and lamellae took the hinge next to the rays, lamellae also the stipe, and the pose nodes gave
 * the margin too. The cap had a seed. From version 8 on the cap grows at points and gives its
 * paths with the rays added, so the stipe runs along with the rays and the nodes after it find
 * where a cap sits from the stipe.
 *
 * A points_on_path with one point at the tip of every path goes in front of every cap. What read
 * the hinge reads those points instead, the hinge links into the pose nodes and lamellae go, and
 * so do the stipe link of lamellae and the seed of the cap. A cap_margin goes after every node
 * whose margin was read. Paths of the cap that went on to other nodes (strands, join, ...) leave
 * the stipe behind through a separate, paths that leave a group keep it. Group inputs and outputs
 * that carried what is gone are removed.
 */
export function capAtPoints(graph: Graph) {
  const before = groupSockets(graph);
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    for (const cap of scope.nodes.filter((n) => n.type === CAP)) {
      const plant = lastEdgeInto(scope, cap.id, 'plant');
      growAtPoints(scope, cap, plant, nextId);
      if (plant) leaveStipe(scope, cap, plant, nextId);
    }
    for (const node of scope.nodes.filter((n) => n.type in MARGIN_OUTPUT)) {
      marginFromRays(scope, node, nextId);
    }
  }
  for (const type of POSE_NODES) removeInputs(graph, type, ['hinge']);
  removeInputs(graph, LAMELLAE, ['hinge', 'stipe']);
  removeInputs(graph, CAP, ['seed']);
  dropDeadSockets(graph, before);
  return graph;
}

/** An input resolves to its last edge at runtime, the editor never makes more than one */
function lastEdgeInto(scope: Scope, node: number, input: string) {
  return scope.edges.filter((e) => e[2] === node && e[3] === input).at(-1);
}

function place(node: SerializedNode, offset: number[]): [number, number] {
  return [node.position[0] + offset[0], node.position[1] + offset[1]];
}

function growAtPoints(
  scope: Scope,
  cap: SerializedNode,
  plant: SerializedEdge | undefined,
  nextId: () => number
) {
  const hinge = (e: SerializedEdge) => e[0] === cap.id && e[1] === HINGE_OUTPUT;
  if (!plant) {
    scope.edges = scope.edges.filter((e) => !hinge(e));
    return;
  }
  const points: SerializedNode = {
    id: nextId(),
    type: POINTS_ON_PATH as SerializedNode['type'],
    position: place(cap, BEFORE),
    props: { amount: 1, lowest_point: 1, highest_point: 1 }
  };
  scope.nodes.push(points);
  scope.edges = scope.edges.map((e): SerializedEdge => hinge(e) ? [points.id, 0, e[2], e[3]] : e);
  scope.edges.push([plant[0], plant[1], points.id, 'plant'], [points.id, 0, cap.id, 'points']);
}

/** Every node the paths of `cap` reach through nodes that pass a cap on */
function chainOf(scope: Scope, cap: SerializedNode) {
  const passes = new Set([FILL_GAPS, ...POSE_NODES]);
  const typeOf = new Map(scope.nodes.map((n) => [n.id, n.type as string]));
  const chain = [cap.id];
  for (let i = 0; i < chain.length; i++) {
    for (const e of scope.edges) {
      if (e[0] !== chain[i] || e[1] !== 0 || chain.includes(e[2])) continue;
      if (passes.has(typeOf.get(e[2]) ?? '')) chain.push(e[2]);
    }
  }
  return chain;
}

function leaveStipe(
  scope: Scope,
  cap: SerializedNode,
  plant: SerializedEdge,
  nextId: () => number
) {
  const typeOf = new Map(scope.nodes.map((n) => [n.id, n.type as string]));
  const chain = chainOf(scope, cap);
  for (const id of chain) {
    const out = scope.edges.filter((e) =>
      e[0] === id && e[1] === 0 && !chain.includes(e[2]) && !TAKES_STIPE.has(typeOf.get(e[2]) ?? '')
    );
    if (out.length === 0) continue;
    const node = scope.nodes.find((n) => n.id === id)!;
    const separate: SerializedNode = {
      id: nextId(),
      type: SEPARATE as SerializedNode['type'],
      position: place(node, AFTER),
      props: {}
    };
    scope.nodes.push(separate);
    scope.edges = scope.edges.map((e): SerializedEdge =>
      out.includes(e) ? [separate.id, 1, e[2], e[3]] : e
    );
    scope.edges.push([id, 0, separate.id, 'plant'], [plant[0], plant[1], separate.id, 'like']);
  }
}

function marginFromRays(scope: Scope, node: SerializedNode, nextId: () => number) {
  const output = MARGIN_OUTPUT[node.type];
  const read = scope.edges.filter((e) => e[0] === node.id && e[1] === output);
  if (read.length === 0) return;

  const margin: SerializedNode = {
    id: nextId(),
    type: CAP_MARGIN as SerializedNode['type'],
    position: place(node, AFTER),
    props: {}
  };
  scope.nodes.push(margin);
  scope.edges = scope.edges.map((e): SerializedEdge =>
    read.includes(e) ? [margin.id, 0, e[2], e[3]] : e
  );
  scope.edges.push([node.id, 0, margin.id, 'cap']);
}

type Sockets = { inputs: Set<string>; outputs: Set<number> };

/** The inputs of every group that a node in it reads, and the outputs read where it is used */
function groupSockets(graph: Graph) {
  const sockets = new Map<number, Sockets>();
  for (const group of graph.groups ?? []) {
    const boundary = group.nodes.find((n) => n.type === GROUP_INPUT);
    const keys = Object.keys(group.inputs ?? {});
    const inputs = new Set(
      group.edges.filter((e) => e[0] === boundary?.id).map((e) => keys[e[1]]).filter((k) => k)
    );
    const outputs = new Set<number>();
    for (const scope of scopes(graph)) {
      const instances = new Set(
        scope.nodes.filter((n) => n.type === GROUP_INSTANCE && n.props?.groupId === group.id).map((
          n
        ) => n.id)
      );
      for (const e of scope.edges) if (instances.has(e[0])) outputs.add(e[1]);
    }
    sockets.set(group.id, { inputs, outputs });
  }
  return sockets;
}

function instancesOf(graph: Graph, group: GroupDefinition) {
  return scopes(graph).map((scope) => ({
    scope,
    ids: new Set(
      scope.nodes.filter((n) => n.type === GROUP_INSTANCE && n.props?.groupId === group.id).map((
        n
      ) => n.id)
    )
  }));
}

function removeGroupInput(graph: Graph, group: GroupDefinition, key: string) {
  const keys = Object.keys(group.inputs ?? {});
  const index = keys.indexOf(key);
  const boundary = group.nodes.find((n) => n.type === GROUP_INPUT);
  group.edges = group.edges.map((e): SerializedEdge =>
    e[0] === boundary?.id && e[1] > index ? [e[0], e[1] - 1, e[2], e[3]] : e
  );
  delete group.inputs![key];
  for (const { scope, ids } of instancesOf(graph, group)) {
    scope.edges = scope.edges.filter((e) => !(ids.has(e[2]) && e[3] === key));
    for (const node of scope.nodes.filter((n) => ids.has(n.id))) {
      if (node.props) delete node.props[key];
      if (node.exposed) node.exposed = node.exposed.filter((k) => k !== key);
    }
  }
}

function removeGroupOutput(graph: Graph, group: GroupDefinition, index: number) {
  const boundary = group.nodes.find((n) => n.type === GROUP_OUTPUT);
  const number = (input: string) => Number(/^out_(\d+)$/.exec(input)?.[1] ?? -1);
  group.edges = group.edges
    .filter((e) => !(e[2] === boundary?.id && number(e[3]) === index))
    .map((e): SerializedEdge =>
      e[2] === boundary?.id && number(e[3]) > index
        ? [e[0], e[1], e[2], `out_${number(e[3]) - 1}`]
        : e
    );
  group.outputs!.splice(index, 1);
  for (const { scope, ids } of instancesOf(graph, group)) {
    scope.edges = scope.edges.map((e): SerializedEdge =>
      ids.has(e[0]) && e[1] > index ? [e[0], e[1] - 1, e[2], e[3]] : e
    );
  }
}

/** Removes the group inputs and outputs that were used before and no longer are */
function dropDeadSockets(graph: Graph, before: Map<number, Sockets>) {
  let changed = true;
  while (changed) {
    changed = false;
    const now = groupSockets(graph);
    for (const group of graph.groups ?? []) {
      const [was, is] = [before.get(group.id), now.get(group.id)];
      if (!was || !is) continue;
      const input = [...was.inputs].find((key) => !is.inputs.has(key));
      if (input !== undefined) {
        removeGroupInput(graph, group, input);
        was.inputs.delete(input);
        changed = true;
        break;
      }
      const output = [...was.outputs].filter((i) => !is.outputs.has(i)).at(-1);
      if (output !== undefined && group.outputs && output < group.outputs.length) {
        removeGroupOutput(graph, group, output);
        was.outputs = new Set(
          [...was.outputs].filter((i) => i !== output).map((i) => i > output ? i - 1 : i)
        );
        changed = true;
        break;
      }
    }
  }
}
