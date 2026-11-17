// Renders graph templates or project files to PNG screenshots, without a browser: `nodarium
// render` of the runtime (packages/runtime/src/cli) with the compiled nodes in static/nodes
// (`pnpm build:nodes`) and the test graphs of the app as templates. Front, side and a view from
// above, next to each other. Stems are brown, instanced geometry (leaves, flowers) is green,
// colors of plants and Myzel meshes show as they are.
//
// Usage: pnpm render-graph <target...> [--out dir] [--size px] [--seed n] [--time t] [--focus x,z,r] [--below] [--node id]
//   target   name of a template in packages/runtime/templates or src/lib/graph-templates (fern,
//            palm, ...), `templates` for all json templates, or the path of a graph or an exported `.nodarium.json` project
//   --out    directory for the screenshots, defaults to ./screenshots
//   --size   size of a single view in pixels, defaults to 512
//   --seed   seed of the random nodes, defaults to 1, the same seed gives the same plant
//   --time   the time of the graph from 0 to 1 (setting time.t), defaults to the graph's own
//   --focus  only renders what lies within r (m) of x, z seen from above, a close-up of one part;
//            the printed size is that of the close-up and the instance count is left out
//   --below  adds a fourth view from below, for the underside of caps and leaves
//   --node   renders what comes out of the node with this id instead of the graph's output
//
// Prints one line of JSON per target with the path of the screenshot and the size of the plant.
// Before a graph runs, the issues validateGraph() finds in it are printed to stderr. A graph with
// an error is not run, its line has the error and all issues. The exit code is 1 once a target
// failed.
import { existsSync } from 'node:fs';
import { readdir } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { main } from '../../packages/runtime/src/cli/main.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
// the templates of the runtime package and the test graphs of the app
const TEMPLATE_DIRS = [
  join(root, '..', 'packages', 'runtime', 'templates'),
  join(root, 'src', 'lib', 'graph-templates')
];

const { values: options, positionals: targets } = parseArgs({
  allowPositionals: true,
  options: {
    out: { type: 'string', default: join(process.cwd(), 'screenshots') },
    size: { type: 'string', default: '512' },
    seed: { type: 'string', default: '1' },
    time: { type: 'string' },
    focus: { type: 'string' },
    below: { type: 'boolean', default: false },
    node: { type: 'string' }
  }
});

if (targets.length === 0) {
  console.error(
    'Usage: pnpm render-graph <template|templates|file.json...> [--out dir] [--size px] [--seed n] [--time t] [--focus x,z,r] [--below] [--node id]'
  );
  process.exit(1);
}

async function resolveTargets(names: string[]) {
  const files: string[] = [];
  for (const name of names) {
    if (name === 'templates') {
      for (const dir of TEMPLATE_DIRS) {
        const templates = (await readdir(dir)).filter((f) => f.endsWith('.json'));
        files.push(...templates.map((f) => join(dir, f)));
      }
    } else if (name.endsWith('.json')) {
      files.push(resolve(name));
    } else {
      const paths = TEMPLATE_DIRS.map((dir) => join(dir, name + '.json'));
      files.push(paths.find((path) => existsSync(path)) ?? paths[0]);
    }
  }
  return files;
}

const flags = Object.entries(options).flatMap(([key, value]) =>
  value === undefined || value === false ? [] : value === true ? [`--${key}`] : [`--${key}`, value]
);

process.exitCode = await main([
  'render',
  ...(await resolveTargets(targets)),
  ...flags,
  '--json',
  '--nodes',
  resolve('static/nodes')
]);
