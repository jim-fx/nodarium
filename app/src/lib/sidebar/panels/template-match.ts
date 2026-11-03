import type { Graph } from '@nodarium/types';

export type TemplateSummary = {
  name: string;
  /** `id:type` of every top level node */
  nodes: string[];
};

/** Below this share of common nodes a graph is not taken for a variant of a template. */
const MIN_OVERLAP = 0.5;

export function nodeSignatures(graph: Pick<Graph, 'nodes'>): string[] {
  return graph.nodes.map((n) => `${n.id}:${n.type}`);
}

export function templateName(title: string): string {
  return title
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

/**
 * The template the graph most likely came from: the one named like its title, otherwise the one
 * that shares the most nodes with it. A project made from a template keeps the ids and types of
 * its nodes, so it is found again after it was renamed or edited.
 */
export function matchTemplate(
  graph: Pick<Graph, 'nodes' | 'meta'>,
  templates: TemplateSummary[]
): string | undefined {
  const title = graph.meta?.title;
  if (title) {
    const byTitle = templates.find((t) => t.name === templateName(title));
    if (byTitle) return byTitle.name;
  }

  const own = new Set(nodeSignatures(graph));
  let best: { name: string; overlap: number } | undefined;
  for (const template of templates) {
    const shared = template.nodes.filter((n) => own.has(n)).length;
    const union = own.size + template.nodes.length - shared;
    const overlap = union === 0 ? 0 : shared / union;
    if (overlap >= MIN_OVERLAP && (!best || overlap > best.overlap)) {
      best = { name: template.name, overlap };
    }
  }
  return best?.name;
}
