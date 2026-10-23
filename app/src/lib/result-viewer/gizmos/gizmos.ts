import type { Edge, Graph, NodeInstance } from '@nodarium/types';
import { pickBranchPath, type ProbePath, sampleAlong } from './gizmo-paths';

export type Vec3 = [number, number, number];

export type GizmoMode = 'translate' | 'scale';

/**
 * What the viewer shows for the active node. The values come from the inputs of the node, and from
 * the incoming paths for the nodes that work on them.
 */
export type Gizmo =
  | { kind: 'force'; nodeId: number; position: Vec3; radius: number }
  | { kind: 'position'; nodeId: number; position: Vec3 }
  | { kind: 'stem'; nodeId: number; origin: Vec3; length?: number }
  | { kind: 'rotate'; nodeId: number; origin: Vec3; axis: 0 | 1 | 2; angle: number }
  | {
    kind: 'branch';
    nodeId: number;
    origin: Vec3;
    /** the direction of the path and a vector orthogonal to it, where the branch starts */
    direction: Vec3;
    orthogonal: Vec3;
    /** degrees, how far the branch leans from the orthogonal towards the direction of the path */
    angle: number;
    length: number;
  };

export type GizmoHandlers = {
  onchange?: (nodeId: number, change: GizmoChange) => void;
  /** a drag started, everything until `onend` is one undo step */
  onstart?: () => void;
  onend?: () => void;
};

/** The props a change in the viewer gives the node */
export type GizmoChange = Record<string, number | number[]>;

export const FORCE_NODE_TYPE = 'max/plantarium/force';
export const VEC3_NODE_TYPE = 'max/plantarium/vec3';
export const STEM_NODE_TYPE = 'max/plantarium/stem';
export const ROTATE_NODE_TYPE = 'max/plantarium/rotate';
export const BRANCH_NODE_TYPE = 'max/plantarium/branch';

export const MIN_GIZMO_RADIUS = 0.05;
export const MIN_LENGTH = 0.05;
export const MAX_BRANCH_ANGLE = 90;

type DefaultValue = (node: NodeInstance, input: string) => unknown;
type EdgeLike = Edge | Graph['edges'][number];

const idOf = (node: number | NodeInstance) => typeof node === 'number' ? node : node.id;

function asVec3(value: unknown): Vec3 | undefined {
  if (!Array.isArray(value) || value.length !== 3) return;
  if (!value.every((v) => typeof v === 'number' && Number.isFinite(v))) return;
  return [value[0], value[1], value[2]];
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

/** The nodes that need to know the paths coming in, and the input those arrive on */
const PROBE_INPUT: Record<string, string> = {
  [ROTATE_NODE_TYPE]: 'plant',
  [BRANCH_NODE_TYPE]: 'plant'
};

/** The node (and its output) that feeds the paths into a node that needs them for its gizmo */
export function probeSource(type: string, nodeId: number, edges: EdgeLike[]) {
  const input = PROBE_INPUT[type];
  if (!input) return;
  const edge = edges.find((e) => idOf(e[2]) === nodeId && e[3] === input);
  if (!edge) return;
  return { nodeId: idOf(edge[0]), output: edge[1] };
}

const DEBUG_NODE_TYPE = '__internal/node/debug';

/**
 * The graph to run so the gizmo of the active node can see the paths that come into it: the same
 * graph with a debug node on the source of those paths. The graph that is saved is left alone.
 * `hidden` means the debug node is only for the gizmo, so its output is not drawn.
 */
export function addProbe(graph: Graph, activeNodeId: number) {
  const node = graph.nodes.find((n) => n.id === activeNodeId);
  const source = node && probeSource(node.type, node.id, graph.edges);
  if (!source) return { graph };

  const hasDebugNode = graph.edges.some(
    (edge) =>
      edge[0] === source.nodeId
      && graph.nodes.find((n) => n.id === edge[2])?.type.endsWith('/debug')
  );
  if (hasDebugNode) return { graph, sourceId: source.nodeId, hidden: false };

  const id = Math.max(0, ...graph.nodes.map((n) => n.id)) + 1;
  return {
    graph: {
      ...graph,
      nodes: [...graph.nodes, { id, type: DEBUG_NODE_TYPE, position: [0, 0], props: {} }],
      edges: [...graph.edges, [source.nodeId, source.output, id, 'input']]
    } as Graph,
    sourceId: source.nodeId,
    hidden: true
  };
}

/** The gizmo of the active node, if it has one and everything it needs is known */
export function collectGizmos(
  nodes: Iterable<NodeInstance>,
  edges: Edge[],
  activeNodeId: number,
  defaultValue: DefaultValue,
  paths: ProbePath[] = []
): Gizmo[] {
  for (const node of nodes) {
    if (node.id !== activeNodeId) continue;

    const connected = (...inputs: string[]) =>
      edges.some((edge) => edge[2].id === node.id && inputs.includes(edge[3]));
    const number = (input: string) =>
      asNumber(node.props?.[input]) ?? asNumber(defaultValue(node, input));

    const gizmo = gizmoBuilders[node.type]?.(node, { connected, number, defaultValue, paths });
    return gizmo ? [gizmo] : [];
  }

  return [];
}

type Context = {
  /** whether any of the inputs gets its value from another node, so there is nothing to edit */
  connected: (...inputs: string[]) => boolean;
  number: (input: string) => number | undefined;
  defaultValue: DefaultValue;
  paths: ProbePath[];
};

const VEC3_INPUTS = ['0', '1', '2'];

function vec3Gizmo(node: NodeInstance, { connected, number }: Context): Gizmo | undefined {
  if (connected(...VEC3_INPUTS)) return;
  const position = VEC3_INPUTS.map((input) => number(input) ?? 0) as Vec3;
  return { kind: 'position', nodeId: node.id, position };
}

function forceGizmo(
  node: NodeInstance,
  { connected, number, defaultValue }: Context
): Gizmo | undefined {
  if (connected('position', 'radius')) return;
  const position = asVec3(node.props?.position) ?? asVec3(defaultValue(node, 'position'));
  const radius = number('radius');
  if (!position || radius === undefined) return;
  return { kind: 'force', nodeId: node.id, position, radius: Math.max(radius, 0) };
}

function stemGizmo(
  node: NodeInstance,
  { connected, number, defaultValue }: Context
): Gizmo | undefined {
  // the length is measured from the origin, without it there is nothing to place
  if (connected('origin')) return;
  const origin = asVec3(node.props?.origin) ?? asVec3(defaultValue(node, 'origin')) ?? [0, 0, 0];
  return {
    kind: 'stem',
    nodeId: node.id,
    origin,
    length: connected('length') ? undefined : number('length')
  };
}

function rotateGizmo(node: NodeInstance, { connected, number, paths }: Context): Gizmo | undefined {
  if (connected('angle')) return;
  // the node turns every path around its first point
  const origin = paths[0]?.points[0];
  if (!origin) return;
  const axis = Math.min(Math.max(Math.round(number('axis') ?? 0), 0), 2) as 0 | 1 | 2;
  return { kind: 'rotate', nodeId: node.id, origin, axis, angle: number('angle') ?? 0 };
}

function branchGizmo(node: NodeInstance, { connected, number, paths }: Context): Gizmo | undefined {
  if (connected('angle', 'length')) return;

  const path = pickBranchPath(paths, Math.max(Math.round(number('depth') ?? 1), 1));
  if (!path) return;

  // the first branch sits at the lowest spot, moved up by the offset of the even branches
  const lowest = number('lowestBranch') ?? 0.2;
  const highest = number('highestBranch') ?? 1;
  const amount = Math.max(number('amount') ?? 1, 1);
  const step = (highest - lowest) / Math.max(amount - 1, 1);
  const alpha = Math.min(Math.max(lowest + (number('offsetSingle') ?? 0) * step, 0), 1);

  const sample = sampleAlong(path.points, alpha);
  if (!sample) return;

  return {
    kind: 'branch',
    nodeId: node.id,
    origin: sample.position,
    direction: sample.tangent,
    orthogonal: sample.orthogonal,
    angle: number('angle') ?? 0,
    length: Math.max(number('length') ?? 0.8, 0)
  };
}

const gizmoBuilders: Record<string, (node: NodeInstance, context: Context) => Gizmo | undefined> = {
  [VEC3_NODE_TYPE]: vec3Gizmo,
  [FORCE_NODE_TYPE]: forceGizmo,
  [STEM_NODE_TYPE]: stemGizmo,
  [ROTATE_NODE_TYPE]: rotateGizmo,
  [BRANCH_NODE_TYPE]: branchGizmo
};

/** Scaling any axis of the gizmo scales the radius, the one that moved the most wins */
export function radiusFromScale(scale: Vec3, startRadius: number) {
  const moved = scale.reduce(
    (best, value) => Math.abs(value - startRadius) > Math.abs(best - startRadius) ? value : best,
    startRadius
  );
  return Math.max(moved, MIN_GIZMO_RADIUS);
}

/** Positions and radii come from dragging, three decimals are plenty */
export function roundGizmoValue(value: number) {
  return Math.round(value * 1000) / 1000;
}
