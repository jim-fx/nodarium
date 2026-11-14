// Renders the previews of the project browser for the templates in packages/runtime/templates into
// static/template-previews, with the thumbnail renderer of the app in a headless Chromium. Needs
// the compiled nodes in static/nodes (`pnpm build:nodes`).
//
// Usage: pnpm render-template-previews [template...] [--all] [--chromium path]
//   without arguments it renders the templates whose preview is missing or stale (the template
//   changed since), `--all` renders all of them, after a change to a node that alters the look.
//   `--chromium` uses that browser instead of the one of Playwright (`pnpm exec playwright install`).
//
// static/template-previews/manifest.json keeps the hash of the template each preview was rendered
// from. The app only uses a preview while that hash matches, a stale one is built live instead.
import { chromium } from '@playwright/test';
import { existsSync } from 'node:fs';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { createServer } from 'vite';
import {
  hashTemplate,
  MANIFEST_FILE,
  PREVIEW_DIR,
  previewFile,
  readManifest,
  readTemplateGraphs
} from './template-previews-plugin.ts';

const PREVIEW_SIZE = 256;
const appDir = dirname(dirname(fileURLToPath(import.meta.url)));

const { values: options, positionals: requested } = parseArgs({
  allowPositionals: true,
  options: { all: { type: 'boolean', default: false }, chromium: { type: 'string' } }
});

if (!existsSync(join(appDir, 'static', 'nodes'))) {
  console.error('static/nodes is missing, run `pnpm build:nodes` first');
  process.exit(1);
}

const templates = await readTemplateGraphs();
const unknown = requested.filter((id) => !templates.some((t) => t.id === id));
if (unknown.length) {
  console.error(`Unknown templates: ${unknown.join(', ')}`);
  process.exit(1);
}

const manifest = await readManifest();
for (const id of Object.keys(manifest)) {
  if (templates.some((t) => t.id === id)) continue;
  delete manifest[id];
  await rm(previewFile(id), { force: true });
  console.log(`removed ${id}`);
}

const stale = templates.filter(({ id, graph }) => {
  if (requested.length) return requested.includes(id);
  return options.all || manifest[id] !== hashTemplate(graph) || !existsSync(previewFile(id));
});

let failed = 0;
if (stale.length) {
  await mkdir(PREVIEW_DIR, { recursive: true });
  const server = await createServer({
    root: appDir,
    logLevel: 'warn',
    server: { port: 0, strictPort: false }
  });
  await server.listen();
  const browser = await chromium.launch({
    executablePath: options.chromium,
    args: ['--enable-unsafe-swiftshader']
  });
  try {
    const page = await browser.newPage();
    page.on('pageerror', (error) => console.error(error));
    await page.goto(new URL('/favicon.svg', server.resolvedUrls!.local[0]).href);
    await page.evaluate(async () => {
      const { createThumbnailWorker } = await import(
        // @ts-expect-error resolved by the dev server
        '/src/lib/thumbnails/thumbnail-worker.ts'
      );
      (globalThis as Record<string, unknown>).thumbnails = createThumbnailWorker();
    });

    for (const { id, graph } of stale) {
      const start = performance.now();
      const data = await page.evaluate(async ({ graph, size }) => {
        const worker = (globalThis as Record<string, unknown>).thumbnails as {
          renderGraph(graph: unknown, size: number): Promise<Blob | undefined>;
        };
        try {
          const blob = await worker.renderGraph(graph, size);
          if (!blob) return { error: 'the graph has no geometry' };
          return { bytes: Array.from(new Uint8Array(await blob.arrayBuffer())) };
        } catch (error) {
          return { error: String(error) };
        }
      }, { graph, size: PREVIEW_SIZE });

      if ('error' in data) {
        console.error(`failed ${id}: ${data.error}`);
        delete manifest[id];
        await rm(previewFile(id), { force: true });
        failed++;
        continue;
      }
      await writeFile(previewFile(id), new Uint8Array(data.bytes!));
      manifest[id] = hashTemplate(graph);
      const ms = Math.round(performance.now() - start);
      console.log(`rendered ${id} (${data.bytes!.length} bytes, ${ms} ms)`);
    }
  } finally {
    await browser.close();
    await server.close();
  }
}

const sorted = Object.fromEntries(Object.entries(manifest).sort(([a], [b]) => a.localeCompare(b)));
await writeFile(MANIFEST_FILE, JSON.stringify(sorted, null, 2) + '\n');
console.log(`${stale.length - failed} rendered, ${failed} failed, ${templates.length} templates`);
if (failed) process.exit(1);
