export const INTERNAL_PREFIX = '__internal/';

export const GROUP_INSTANCE = '__internal/group/instance';
export const GROUP_INPUT = '__internal/group/input';
export const GROUP_OUTPUT = '__internal/group/output';
export const DEBUG_NODE = '__internal/node/debug';

type NodeLike = { type: string; props?: Record<string, unknown> };

/** The id of the group a group instance points to, `undefined` for any other node */
export function groupIdOf(node: NodeLike | undefined): number | undefined {
  if (node?.type !== GROUP_INSTANCE) return undefined;
  return node.props?.groupId as number | undefined;
}
