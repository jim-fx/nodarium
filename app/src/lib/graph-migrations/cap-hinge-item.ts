import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, removeInputs, type Scope, scopes } from './helpers';

const CAP = 'max/myzel/cap';
const CAP_MARGIN = 'max/myzel/cap_margin';
const LAMELLAE = 'max/myzel/lamellae';

/** The nodes that posed a cap from its rays and hinge and gave its margin as output 1 */
const POSE_NODES = ['cap_lopsided', 'cap_open', 'margin_curl', 'margin_waves'].map((name) =>
  `max/myzel/${name}`
);

/** The output the margin was, on the cap and on the pose nodes */
const MARGIN_OUTPUT: Record<string, number> = {
  [CAP]: 2,
  ...Object.fromEntries(POSE_NODES.map((type) => [type, 1]))
};

/** Where cap_margin sits, from the node whose rays it reads */
const OFFSET = [15, 25];

/**
 * Up to graph version 7 the pose nodes (margin_waves, cap_lopsided, margin_curl, cap_open) and
 * lamellae took the hinge output of the cap next to its rays, and the cap and every pose node gave
 * the margin as points. From version 8 on the rays carry their hinge, so the hinge links go, and
 * cap_margin makes the margin from the rays: one is put after every node whose margin was read.
 * The seed of the cap is gone, the path id of the stipe moves its rays.
 */
export function capHingeItem(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) {
    for (const node of scope.nodes.filter((n) => n.type in MARGIN_OUTPUT)) {
      marginFromRays(scope, node, nextId);
    }
  }
  for (const type of [...POSE_NODES, LAMELLAE]) removeInputs(graph, type, ['hinge']);
  return removeInputs(graph, CAP, ['seed']);
}

function marginFromRays(scope: Scope, node: SerializedNode, nextId: () => number) {
  const output = MARGIN_OUTPUT[node.type];
  const read = scope.edges.filter((e) => e[0] === node.id && e[1] === output);
  if (read.length === 0) return;

  const margin: SerializedNode = {
    id: nextId(),
    type: CAP_MARGIN as SerializedNode['type'],
    position: [node.position[0] + OFFSET[0], node.position[1] + OFFSET[1]],
    props: {}
  };
  scope.nodes.push(margin);
  scope.edges = scope.edges.map((e): SerializedEdge =>
    read.includes(e) ? [margin.id, 0, e[2], e[3]] : e
  );
  scope.edges.push([node.id, 0, margin.id, 'cap']);
}
