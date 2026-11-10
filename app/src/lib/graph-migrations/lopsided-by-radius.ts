import type { Graph, SerializedEdge } from '@nodarium/types';
import { type Scope, scopes } from './helpers';

const CAP = 'max/myzel/cap';
const CAP_LOPSIDED = 'max/myzel/cap_lopsided';
const RANDOM = 'max/plantarium/random';

/**
 * Up to graph version 8 cap_lopsided made a cap reach further on one side and tilted it. From
 * version 9 on the cap reads its radius for every ray, with alpha once around the axis, so a
 * curve into the radius makes it lopsided, and the stipe tip tilts it. Every cap_lopsided goes,
 * the paths it got go on to the nodes it fed, the cap becomes round.
 *
 * A random node into the radius drew one value for every cap, now it would draw one for every
 * ray and make the margin ragged. The random nodes that feed the radius of a cap draw once per
 * path instead, so the cap stays round.
 */
export function lopsidedByRadius(graph: Graph) {
  for (const scope of scopes(graph)) {
    for (const node of scope.nodes.filter((n) => n.type === CAP_LOPSIDED)) bypass(scope, node.id);
    for (const cap of scope.nodes.filter((n) => n.type === CAP)) drawOncePerPath(scope, cap.id);
  }
  return graph;
}

function bypass(scope: Scope, id: number) {
  // an input resolves to its last edge at runtime, the editor never makes more than one
  const into = scope.edges.filter((e) => e[2] === id && e[3] === 'cap').at(-1);
  scope.edges = scope.edges.flatMap((e): SerializedEdge[] => {
    if (e[2] === id) return [];
    if (e[0] !== id) return [e];
    return into ? [[into[0], into[1], e[2], e[3]]] : [];
  });
  scope.nodes = scope.nodes.filter((n) => n.id !== id);
}

function drawOncePerPath(scope: Scope, cap: number) {
  const seen = new Set<number>();
  const queue = scope.edges.filter((e) => e[2] === cap && e[3] === 'radius').map((e) => e[0]);
  while (queue.length > 0) {
    const id = queue.pop()!;
    if (seen.has(id)) continue;
    seen.add(id);
    const node = scope.nodes.find((n) => n.id === id);
    if (node?.type === RANDOM) node.props = { ...node.props, per_path: 1 };
    queue.push(...scope.edges.filter((e) => e[2] === id).map((e) => e[0]));
  }
}
