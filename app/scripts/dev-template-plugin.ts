// Dev server only: lets the editor write the open graph into one of the json files in
// src/lib/graph-templates. A name that doesn't exist yet creates the file and exports it from
// index.ts.
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const TEMPLATE_DIR = join(
  dirname(dirname(fileURLToPath(import.meta.url))),
  'src/lib/graph-templates'
);
export const TEMPLATE_ENDPOINT = '/__dev/templates';
const NAME_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;

async function listTemplates() {
  return (await readdir(TEMPLATE_DIR)).filter((f) => f.endsWith('.json')).map((f) =>
    f.slice(0, -5)
  );
}

// adds `export { default as <camelName> } from './<name>.json';` in alphabetical order
async function exportTemplate(name: string) {
  const indexFile = join(TEMPLATE_DIR, 'index.ts');
  const camel = name.replace(/-([a-z0-9])/g, (_, c) => c.toUpperCase());
  const line = `export { default as ${camel} } from './${name}.json';`;
  const lines = (await readFile(indexFile, 'utf-8')).split('\n');
  const path = (l: string) => l.match(/from '\.\/(.+?)\.(json|ts)'/)?.[1];
  let at = lines.findIndex((l) => l.startsWith('export') && path(l)! > name);
  if (at === -1) at = lines.findLastIndex((l) => l.startsWith('export')) + 1;
  lines.splice(at, 0, line);
  await writeFile(indexFile, lines.join('\n'));
}

export function devTemplatePlugin(): Plugin {
  return {
    name: 'nodarium-dev-templates',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use(TEMPLATE_ENDPOINT, async (req, res) => {
        const send = (status: number, body: unknown) => {
          res.statusCode = status;
          res.setHeader('Content-Type', 'application/json');
          res.end(JSON.stringify(body));
        };
        try {
          if (req.method === 'GET') return send(200, await listTemplates());
          if (req.method !== 'POST') return send(405, { error: 'Method not allowed' });

          let raw = '';
          for await (const chunk of req) raw += chunk;
          const { name, graph } = JSON.parse(raw);
          if (typeof name !== 'string' || !NAME_PATTERN.test(name)) {
            return send(400, { error: 'Use lowercase letters, digits and dashes for the name' });
          }
          if (!graph || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) {
            return send(400, { error: 'Not a graph' });
          }
          const created = !(await listTemplates()).includes(name);
          await writeFile(
            join(TEMPLATE_DIR, `${name}.json`),
            JSON.stringify(graph, null, 2) + '\n'
          );
          if (created) await exportTemplate(name);
          send(200, { ok: true, created });
        } catch (e) {
          send(500, { error: e instanceof Error ? e.message : String(e) });
        }
      });
    }
  };
}
