export const INTERNAL_PREFIX = '__internal/';

export const GROUP_INSTANCE = '__internal/group/instance';
export const GROUP_INPUT = '__internal/group/input';
export const GROUP_OUTPUT = '__internal/group/output';
export const DEBUG_NODE = '__internal/node/debug';

/** A node whose input is the result of its graph, like `max/core/output` */
export function isOutputNode(type: string) {
  return type.endsWith('/output');
}

/** A node that shows the output connected to it in the viewer, like {@link DEBUG_NODE} */
export function isDebugNode(type: string) {
  return type.endsWith('/debug');
}

type NodeLike = { type: string; props?: Record<string, unknown> };
type GroupLike = { inputs?: Record<string, { type: string; value?: unknown }> };

/** The id of the group a group instance points to, `undefined` for any other node */
export function groupIdOf(node: NodeLike | undefined): number | undefined {
  if (node?.type !== GROUP_INSTANCE) return undefined;
  return node.props?.groupId as number | undefined;
}

/**
 * The key of the group input behind output socket `index` of the Group Input node. The sockets
 * are positional: after an input was removed the keys have a gap, the sockets don't.
 */
export function groupInputKey(group: GroupLike, index: number): string | undefined {
  // no array of the keys: the runtime asks once per edge that leaves the Group Input node
  let socket = 0;
  for (const key in group.inputs) if (socket++ === index) return key;
  return undefined;
}

/**
 * The output of the group that an input of the Group Output node stands for: `out_N` is
 * output N, older groups named their single output `Out`
 */
export function groupOutputIndex(socket: string): number {
  return Number(/^out_(\d+)$/.exec(socket)?.[1] ?? 0);
}

/**
 * The value an unconnected input of a group instance runs with: the one set on the instance,
 * else the default of the group input, which the widget of the instance shows
 */
export function groupInputValue(instance: NodeLike, group: GroupLike, key: string): unknown {
  return instance.props?.[key] ?? group.inputs?.[key]?.value;
}
