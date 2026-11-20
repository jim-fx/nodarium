import { track } from '#lib/analytics.ts';
import type { UrlOptions } from '#lib/url-options.ts';
import { decodeGraph } from '@nodarium/runtime';
import { templates } from '@nodarium/runtime/templates';
import { toast } from '@nodarium/ui';
import type { ProjectManager } from './project-manager.svelte';

async function readTemplate(id: string) {
  const template = templates.find((t) => t.id === id);
  if (!template) throw new Error(`there is no template "${id}"`);
  return { input: await template.load(), title: template.name };
}

/**
 * Opens the template or the graph of a link, as File → Import does with a file. A project with
 * the same graph is opened instead of a new one. A link that can't be opened shows a toast with
 * the reason and opens nothing.
 */
export async function openUrlProject(
  pm: ProjectManager,
  { template, graph, seed }: UrlOptions
): Promise<number | undefined> {
  if (graph === undefined && template === undefined) return;
  try {
    const { input, title } = graph === undefined
      ? await readTemplate(template!)
      : { input: await decodeGraph(graph), title: undefined };
    const { id } = await pm.importGraph(input, {
      title,
      fallbackTitle: 'Linked graph',
      settings: seed === undefined ? undefined : { seed },
      reuse: true
    });
    if (graph === undefined) track('template-open', { template: template!, source: 'url' });
    else track('share-link', { action: 'open', ok: true });
    return id;
  } catch (error) {
    if (graph === undefined) track('template-open', { template: 'unknown', source: 'url' });
    else track('share-link', { action: 'open', ok: false });
    const message = error instanceof Error ? error.message : String(error);
    toast(`Opening the link failed: ${message}`, 'error', 8000);
  }
}
