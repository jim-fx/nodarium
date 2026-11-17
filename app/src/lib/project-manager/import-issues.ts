import { errorLog } from '#lib/runtime/error-log.svelte.ts';
import { type GraphIssue, validateGraph } from '@nodarium/runtime';
import { type Graph, INTERNAL_PREFIX, type NodeId, type NodeRegistry } from '@nodarium/types';
import { toast } from '@nodarium/ui';

/**
 * The issues of a graph as it is written in an imported file. The collections of its nodes are
 * loaded, so a mistyped node type finds the node it meant.
 */
export async function findImportIssues(graph: Graph, registry: NodeRegistry) {
  const types = [...graph.nodes, ...(graph.groups ?? []).flatMap((g) => g.nodes)]
    .map((node) => node.type)
    .filter((type) => !type.startsWith(INTERNAL_PREFIX));
  const collections = new Set(types.map((type) => type.split('/').slice(0, 2).join('/')));
  await Promise.allSettled(
    [...collections].map((c) => registry.loadCollection?.(c as `${string}/${string}`))
  );
  // a node type the registry doesn't have is one of the issues
  await Promise.allSettled(
    [...new Set(types)].map((type) =>
      registry.loadDefinitions?.([type as NodeId]) ?? registry.load([type as NodeId])
    )
  );
  const definitions = new Map(registry.getAllNodes().map((d) => [d.id as string, d]));
  return validateGraph(graph, definitions);
}

/** A toast with the first issue and how many there are, all of them in the error log */
export function reportImportIssues(graph: Graph, issues: GraphIssue[]) {
  if (issues.length === 0) return;
  const types = new Map(graph.nodes.map((node) => [node.id, node.type]));
  // newest first, the first issue ends up on top
  for (const issue of [...issues].reverse()) {
    // a node inside a group can't be focused from the graph
    const nodeId = issue.groupId === undefined ? issue.nodeId : undefined;
    const type = nodeId === undefined ? undefined : types.get(nodeId);
    errorLog.add({
      message: issue.message,
      nodeId: type ? nodeId : undefined,
      nodeType: type,
      nodeTitle: type?.split('/').pop()
    });
  }
  const errors = issues.filter((issue) => issue.severity === 'error');
  const first = errors[0] ?? issues[0];
  const count = issues.length === 1 ? '1 issue' : `${issues.length} issues`;
  toast(
    `The imported graph has ${count}: ${first.message}`,
    errors.length ? 'error' : 'info',
    8000
  );
}
