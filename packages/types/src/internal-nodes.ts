export const INTERNAL_PREFIX = '__internal/';

export const GROUP_INSTANCE = '__internal/group/instance';
export const GROUP_INPUT = '__internal/group/input';
export const GROUP_OUTPUT = '__internal/group/output';
export const DEBUG_NODE = '__internal/node/debug';

/** A node whose input is the result of its graph, like `max/plantarium/output` */
export function isOutputNode(type: string) {
  return type.endsWith('/output');
}

/** A node that shows the output connected to it in the viewer, like {@link DEBUG_NODE} */
export function isDebugNode(type: string) {
  return type.endsWith('/debug');
}

type NodeLike = { type: string; props?: Record<string, unknown> };

/** The id of the group a group instance points to, `undefined` for any other node */
export function groupIdOf(node: NodeLike | undefined): number | undefined {
  if (node?.type !== GROUP_INSTANCE) return undefined;
  return node.props?.groupId as number | undefined;
}
