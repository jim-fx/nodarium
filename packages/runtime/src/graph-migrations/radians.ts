import type { Graph, SerializedEdge, SerializedNode } from '@nodarium/types';
import { idAllocator, type Scope, scopes } from './helpers';

const DEGREES = Math.PI / 180;
const TURNS = 2 * Math.PI;

/** The inputs that took degrees or turns up to graph version 13, and what turns them to radians */
export const ANGLE_INPUTS: Record<string, Record<string, number>> = {
  'max/plantarium/branch': { rotation: DEGREES, angle: DEGREES },
  'max/plantarium/branch_from_points': { angle: DEGREES },
  'max/plantarium/trunk_kink': { angle: DEGREES, rotation: DEGREES },
  'max/plantarium/umbel': { opening: DEGREES },
  'max/plantarium/instance': { angle: DEGREES },
  'max/plantarium/instance_from_points': { angle: DEGREES },
  'max/plantarium/points_on_path': { rotation: DEGREES },
  'max/plantarium/stem': { fan: DEGREES },
  'max/plantarium/leaf_growth': { vein_angle: DEGREES, x_curve: DEGREES, y_curve: DEGREES },
  'max/plantarium/deform': { amount: TURNS }
};

/**
 * Nodes whose value only scales with some of their own props, by those props and the defaults of
 * the nodes, which a graph leaves out
 */
const SCALABLE: Record<string, Record<string, number>> = {
  'max/plantarium/random': { min: 1, max: 2 },
  'max/plantarium/curve': { min: 0, max: 1 },
  'max/plantarium/float': { value: 0.1 }
};

const MATH = 'max/plantarium/math';

/** Enough digits that a plant doesn't move, few enough to read */
const round = (v: number) => Math.round(v * 1e7) / 1e7;

/**
 * Up to graph version 13 most angles were degrees: the rotation and angle of branch, the angle of
 * instance, the fan of stem and others, and deform took turns. From version 14 on every angle is
 * radians. A value set on a node is converted. A value that comes over an edge is converted where
 * it starts when nothing else reads it: the min and max of a random or a curve and the value of a
 * float that only feed angles of one kind are scaled. Any other source keeps its value and gets a
 * math node that multiplies it on the way to the angles, so the other inputs it feeds stay as
 * they were.
 */
export function radians(graph: Graph) {
  const nextId = idAllocator(graph);
  for (const scope of scopes(graph)) convertScope(scope, nextId);
  return graph;
}

function convertScope(scope: Scope, nextId: () => number) {
  const byId = new Map(scope.nodes.map((n) => [n.id, n]));
  const factorOf = (edge: SerializedEdge) => ANGLE_INPUTS[byId.get(edge[2])?.type ?? '']?.[edge[3]];

  for (const node of scope.nodes) {
    const inputs = ANGLE_INPUTS[node.type];
    if (!inputs || !node.props) continue;
    for (const [key, factor] of Object.entries(inputs)) {
      const value = node.props[key];
      if (typeof value === 'number') node.props[key] = round(value * factor);
    }
  }

  const sources = new Map<string, SerializedEdge[]>();
  for (const edge of scope.edges) {
    if (factorOf(edge) === undefined) continue;
    const key = `${edge[0]}:${edge[1]}`;
    sources.set(key, [...(sources.get(key) ?? []), edge]);
  }

  for (const angleEdges of sources.values()) {
    const [from, output] = angleEdges[0];
    const source = byId.get(from);
    const all = scope.edges.filter((e) => e[0] === from && e[1] === output);
    const factors = new Set(angleEdges.map(factorOf));
    const scalable = source && SCALABLE[source.type];
    const wired = (key: string) => scope.edges.some((e) => e[2] === from && e[3] === key);
    if (
      source && scalable && all.length === angleEdges.length && factors.size === 1
      && !Object.keys(scalable).some(wired)
    ) {
      const [factor] = factors;
      source.props ??= {};
      for (const [key, fallback] of Object.entries(scalable)) {
        const value = source.props[key];
        source.props[key] = round((typeof value === 'number' ? value : fallback) * factor!);
      }
      continue;
    }
    for (const factor of factors) {
      const id = nextId();
      const into = angleEdges.filter((e) => factorOf(e) === factor);
      const target = byId.get(into[0][2]);
      const math: SerializedNode = {
        id,
        type: MATH,
        props: { op_type: 2, b: factor! }
      };
      if (source?.position && target?.position) {
        math.position = [
          (source.position[0] + target.position[0]) / 2,
          (source.position[1] + target.position[1]) / 2
        ];
      }
      scope.nodes.push(math);
      scope.edges = scope.edges.map((e) => (into.includes(e) ? [id, 0, e[2], e[3]] : e));
      scope.edges.push([from, output, id, 'a']);
    }
  }
}
