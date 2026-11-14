// Provides `virtual:template-previews`: the urls of the pre-rendered template previews in
// static/template-previews that still match their template. The hashes are compared when the app
// is built (or the dev server loads the module), a template whose preview is stale or missing is
// left out and the project browser builds its preview live. `pnpm render-template-previews`
// renders the stale ones again.
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { readdir, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';
import {
  selectStaticPreviews,
  TEMPLATE_PREVIEW_DIR,
  type TemplatePreviewManifest
} from '../src/lib/project-manager/template-previews.ts';

const appDir = dirname(dirname(fileURLToPath(import.meta.url)));
export const TEMPLATE_DIR = join(appDir, '../packages/runtime/templates');
export const PREVIEW_DIR = join(appDir, 'static', TEMPLATE_PREVIEW_DIR);
export const MANIFEST_FILE = join(PREVIEW_DIR, 'manifest.json');

const MODULE_ID = 'virtual:template-previews';
const RESOLVED_ID = '\0' + MODULE_ID;

type TemplateNode = { id: number; type: string; props?: unknown };
type TemplateGraph = {
  nodes: TemplateNode[];
  edges: unknown[];
  groups?: { nodes: TemplateNode[] }[];
  settings?: unknown;
};

// The same parts as hashGraph of the thumbnails, moving a node doesn't change the preview.
// Its own hash, the config of vite can't import the sources of @nodarium/utils.
export function hashTemplate(graph: TemplateGraph) {
  const strip = (nodes: TemplateNode[]) =>
    nodes.map(({ id, type, props }) => ({ id, type, props }));
  const json = JSON.stringify({
    nodes: strip(graph.nodes),
    edges: graph.edges,
    groups: graph.groups?.map((group) => ({ ...group, nodes: strip(group.nodes) })),
    settings: graph.settings ?? {}
  });
  return createHash('sha256').update(json).digest('hex').slice(0, 16);
}

export async function readTemplateGraphs() {
  const files = (await readdir(TEMPLATE_DIR)).filter((f) => f.endsWith('.json')).sort();
  return Promise.all(files.map(async (file) => ({
    id: file.slice(0, -5),
    file: join(TEMPLATE_DIR, file),
    graph: JSON.parse(await readFile(join(TEMPLATE_DIR, file), 'utf-8')) as TemplateGraph
  })));
}

export async function readManifest(): Promise<TemplatePreviewManifest> {
  if (!existsSync(MANIFEST_FILE)) return {};
  return JSON.parse(await readFile(MANIFEST_FILE, 'utf-8'));
}

export function previewFile(id: string) {
  return join(PREVIEW_DIR, `${id}.webp`);
}

export function templatePreviewsPlugin(): Plugin {
  return {
    name: 'nodarium-template-previews',
    resolveId(id) {
      if (id === MODULE_ID) return RESOLVED_ID;
    },
    async load(id) {
      if (id !== RESOLVED_ID) return;
      const templates = await readTemplateGraphs();
      const manifest = await readManifest();
      this.addWatchFile(MANIFEST_FILE);
      const hashes: Record<string, string> = {};
      for (const { id, file, graph } of templates) {
        this.addWatchFile(file);
        if (existsSync(previewFile(id))) hashes[id] = hashTemplate(graph);
      }
      return `export default ${JSON.stringify(selectStaticPreviews(manifest, hashes))};`;
    }
  };
}
