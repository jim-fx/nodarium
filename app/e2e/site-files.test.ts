// The files for agents and tools are in the build the e2e tests run against, Playwright builds it
// first (CI builds it in a step before)
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const BUILD = join(import.meta.dirname, '../build');

const read = (path: string) => readFileSync(join(BUILD, path), 'utf8');

/** The file of the build a path of the site is served from */
function builtFile(path: string) {
  const clean = path.split(/[?#]/)[0];
  return join(BUILD, clean.endsWith('/') ? `${clean}index.html` : clean);
}

test('every link of llms.txt and of the page without JavaScript is in the build', () => {
  const llms = read('llms.txt');
  const links = [...llms.matchAll(/\]\((\/[^)]*)\)/g)].map((m) => m[1]);
  expect(links.length).toBeGreaterThanOrEqual(6);
  const page = read('index.html');
  const noscript = page.match(/<noscript>([\s\S]*?)<\/noscript>/)![1];
  links.push(...[...noscript.matchAll(/href="(\/[^"]*)"/g)].map((m) => m[1]));
  expect(page).toContain('<link rel="alternate" type="text/plain" href="/llms.txt"');
  expect(page).toMatch(/<meta\s+name="description"/);

  expect(links.filter((link) => !existsSync(builtFile(link)))).toEqual([]);
});

test('the catalog and the templates point to files of the build', () => {
  const catalog = JSON.parse(read('nodes/index.json'));
  expect(catalog.nodes.length).toBeGreaterThan(20);
  const templates = JSON.parse(read('templates/index.json'));
  expect(templates.map((t: { id: string }) => t.id)).toContain('fern');
  const paths = [
    ...catalog.nodes.map((n: { definition: string }) => n.definition),
    ...templates.flatMap((t: { graph: string; preview?: string }) => [t.graph, t.preview ?? []])
      .flat()
  ];
  expect(paths.filter((path) => !existsSync(builtFile(path)))).toEqual([]);
  expect(read('nodes/index.txt')).toContain('max/plantarium/stem (points) -> path');
});

test('the files are served', async ({ request }) => {
  for (const path of ['/llms.txt', '/graph.schema.json', '/templates/fern.json']) {
    const response = await request.get(`http://localhost:4173${path}`);
    expect(response.status(), path).toBe(200);
  }
  const schema = await (await request.get('http://localhost:4173/graph.schema.json')).json();
  expect(Object.keys(schema.$defs)).toEqual(['ProjectFile', 'Graph']);
});
