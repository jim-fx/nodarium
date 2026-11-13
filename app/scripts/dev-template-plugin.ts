// Dev server only: lets the editor write the open graph into one of the json templates, the ones of
// packages/runtime/templates or the test graphs in src/lib/graph-templates. A name that doesn't
// exist yet becomes a template of the runtime package and is added to its index.
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const appDir = dirname(dirname(fileURLToPath(import.meta.url)));
const RUNTIME_TEMPLATE_DIR = join(appDir, '../packages/runtime/templates');
const TEMPLATE_DIRS = [RUNTIME_TEMPLATE_DIR, join(appDir, 'src/lib/graph-templates')];
export const TEMPLATE_ENDPOINT = '/__dev/templates';
const NAME_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;

async function listTemplates() {
  const templates = new Map<string, string>();
  for (const dir of TEMPLATE_DIRS) {
    for (const file of await readdir(dir)) {
      if (file.endsWith('.json')) templates.set(file.slice(0, -5), join(dir, file));
    }
  }
  return templates;
}

// the name and the `id:type` of every node, so the editor can tell which template a graph came from
async function summarizeTemplates() {
  return Promise.all(
    [...(await listTemplates())].map(async ([name, file]) => {
      const graph = JSON.parse(await readFile(file, 'utf-8'));
      const nodes: { id: number; type: string }[] = graph.nodes ?? [];
      return { name, nodes: nodes.map((n) => `${n.id}:${n.type}`) };
    })
  ).then((summaries) => summaries.sort((a, b) => a.name.localeCompare(b.name)));
}

// adds `plant('<name>', '<Name>', '<unit>', () => import('./<name>.json')),` to the end of the list
async function registerTemplate(
  name: string,
  graph: { nodes: { type: string }[]; meta?: { unit?: string } }
) {
  const indexFile = join(RUNTIME_TEMPLATE_DIR, 'index.ts');
  const kind = graph.nodes.some((n) => n.type === 'max/myzel/output') ? 'mushroom' : 'plant';
  const title = name.replace(/-/g, ' ').replace(/^./, (c) => c.toUpperCase());
  const unit = graph.meta?.unit ?? 'm';
  const line = `  ${kind}('${name}', '${title}', '${unit}', () => import('./${name}.json'))`;
  const lines = (await readFile(indexFile, 'utf-8')).split('\n');
  const end = lines.indexOf('];', lines.findIndex((l) => l.startsWith('export const templates')));
  lines[end - 1] += ',';
  lines.splice(end, 0, line);
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
          if (req.method === 'GET') return send(200, await summarizeTemplates());
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
          const existing = (await listTemplates()).get(name);
          await writeFile(
            existing ?? join(RUNTIME_TEMPLATE_DIR, `${name}.json`),
            JSON.stringify(graph, null, 2) + '\n'
          );
          if (!existing) await registerTemplate(name, graph);
          send(200, { ok: true, created: !existing });
        } catch (e) {
          send(500, { error: e instanceof Error ? e.message : String(e) });
        }
      });
    }
  };
}
