import { graphLinkData } from '@nodarium/runtime';

/** What the address of the page asks for, read once when the page loads */
export type UrlOptions = {
  /** `?tutorial=0` keeps Planty from starting on its own for this visit */
  tutorial: boolean;
  /** `?template=fern` opens a template as a new project */
  template?: string;
  /** `#graph=<data>` opens the graph of a link as a new project, see `graphLink()` */
  graph?: string;
  /** `?seed=7` is the seed of the graph that `template` or `graph` opens */
  seed?: number;
  /** `?panel=graph-settings` opens a panel of the sidebar */
  panel?: string;
};

const SEARCH_KEYS = ['tutorial', 'template', 'seed', 'panel'];

export function readUrlOptions(url: URL): UrlOptions {
  const params = url.searchParams;
  const seed = params.get('seed');
  return {
    tutorial: params.get('tutorial') !== '0',
    template: params.get('template') || undefined,
    graph: graphLinkData(url.href),
    seed: seed !== null && /^\d{1,9}$/.test(seed) ? Number(seed) : undefined,
    panel: params.get('panel') || undefined
  };
}

/**
 * The search and the fragment of the address without the options, so a reload doesn't open the
 * graph again and the graph of a link doesn't stay in the address bar. Undefined when it has none.
 */
export function withoutUrlOptions(url: URL): '' | `?${string}` | `#${string}` | undefined {
  const clean = new URL(url);
  for (const key of SEARCH_KEYS) clean.searchParams.delete(key);
  if (graphLinkData(url.href) !== undefined) clean.hash = '';
  const rest = (clean.search + clean.hash) as '' | `?${string}` | `#${string}`;
  return rest === url.search + url.hash ? undefined : rest;
}
