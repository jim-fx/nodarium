import type { Edge, NodeInstance } from '@nodarium/types';

export type Vec3 = [number, number, number];

export type GizmoMode = 'translate' | 'scale';

/** A position in the viewer that comes from the inputs of the active node, a point force also has a radius */
export type ForceGizmo = {
  nodeId: number;
  position: Vec3;
  /** undefined for a plain position, which can't be scaled */
  radius?: number;
};

export type GizmoChange = { position?: Vec3; radius?: number };

export const FORCE_NODE_TYPE = 'max/plantarium/force';
export const VEC3_NODE_TYPE = 'max/plantarium/vec3';
const VEC3_INPUTS = ['0', '1', '2'];
export const MIN_GIZMO_RADIUS = 0.05;

function asVec3(value: unknown): Vec3 | undefined {
  if (!Array.isArray(value) || value.length !== 3) return;
  if (!value.every((v) => typeof v === 'number' && Number.isFinite(v))) return;
  return [value[0], value[1], value[2]];
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

/**
 * The gizmo of the active node, if it is a force or vec3 node. A node whose position or radius
 * comes from another node has no value to edit here, so it gets none.
 */
export function collectForceGizmos(
  nodes: Iterable<NodeInstance>,
  edges: Edge[],
  activeNodeId: number,
  defaultValue: (node: NodeInstance, input: string) => unknown
): ForceGizmo[] {
  const gizmos: ForceGizmo[] = [];

  for (const node of nodes) {
    if (node.id !== activeNodeId) continue;

    if (node.type === VEC3_NODE_TYPE) {
      const gizmo = vec3Gizmo(node, edges, defaultValue);
      if (gizmo) gizmos.push(gizmo);
      continue;
    }
    if (node.type !== FORCE_NODE_TYPE) continue;
    if (
      edges.some((edge) =>
        edge[2].id === node.id && (edge[3] === 'position' || edge[3] === 'radius')
      )
    ) {
      continue;
    }

    const position = asVec3(node.props?.position) ?? asVec3(defaultValue(node, 'position'));
    const radius = asNumber(node.props?.radius) ?? asNumber(defaultValue(node, 'radius'));
    if (!position || radius === undefined) continue;

    gizmos.push({
      nodeId: node.id,
      position,
      radius: Math.max(radius, 0)
    });
  }

  return gizmos;
}

function vec3Gizmo(
  node: NodeInstance,
  edges: Edge[],
  defaultValue: (node: NodeInstance, input: string) => unknown
): ForceGizmo | undefined {
  if (edges.some((edge) => edge[2].id === node.id && VEC3_INPUTS.includes(edge[3]))) return;

  const position = VEC3_INPUTS.map(
    (input) => asNumber(node.props?.[input]) ?? asNumber(defaultValue(node, input)) ?? 0
  ) as Vec3;
  return { nodeId: node.id, position };
}

/** The props a node has to be given for a change made in the viewer */
export function gizmoProps(
  node: NodeInstance,
  change: GizmoChange
): Record<string, number | number[]> {
  const { position, radius } = change;
  if (node.type === VEC3_NODE_TYPE) {
    return position ? Object.fromEntries(VEC3_INPUTS.map((input, i) => [input, position[i]])) : {};
  }
  return {
    ...(position && { position }),
    ...(radius !== undefined && { radius })
  };
}

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
