// Lays out the nodes of graph templates or project files in columns, like "Format nodes" in the
// node editor. Only the positions in the file change, the rest of it stays as it is written.
// It needs the compiled nodes in static/nodes (`pnpm build:nodes`) to know how tall a node is.
//
// Usage: pnpm format-graph <target...> [--check]
//   target   name of a template in src/lib/graph-templates (fern, palm, ...), `templates` for
//            all json templates, or the path of a graph or an exported `.nodarium.json` project
//   --check  only report the files whose layout would change, and exit with 1 if there are any
import type { NodeId, SerializedEdge, SerializedNode } from '@nodarium/types';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { BenchmarkRegistry } from '../benchmark/benchmarkRegistry.ts';
import { layoutGraph } from '../src/lib/graph-interface/helpers/layoutGraph.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const TEMPLATE_DIR = join(root, 'src', 'lib', 'graph-templates');

const { values: options, positionals: targets } = parseArgs({
  allowPositionals: true,
  options: {
    check: { type: 'boolean', default: false }
  }
});

if (targets.length === 0) {
  console.error('Usage: pnpm format-graph <template|templates|file.json...> [--check]');
  process.exit(1);
}

async function resolveTargets(names: string[]) {
  const files: string[] = [];
  for (const name of names) {
    if (name === 'templates') {
      const templates = (await readdir(TEMPLATE_DIR)).filter((f) => f.endsWith('.json'));
      files.push(...templates.map((f) => join(TEMPLATE_DIR, f)));
    } else if (name.endsWith('.json')) {
      files.push(resolve(name));
    } else {
      files.push(join(TEMPLATE_DIR, name + '.json'));
    }
  }
  return files;
}

type Level = { nodes: SerializedNode[]; edges: SerializedEdge[] };

// the root graph and every group in it, wherever they sit in the file
function findLevels(value: unknown, levels: Level[] = []): Level[] {
  if (Array.isArray(value)) {
    for (const v of value) findLevels(v, levels);
  } else if (value && typeof value === 'object') {
    if ('nodes' in value && Array.isArray(value.nodes) && 'edges' in value) {
      levels.push(value as Level);
    }
    for (const v of Object.values(value)) findLevels(v, levels);
  }
  return levels;
}

// every value of a "position" key, in the order they are written in the file
function findPositions(value: unknown, positions: unknown[] = []): unknown[] {
  if (Array.isArray(value)) {
    for (const v of value) findPositions(v, positions);
  } else if (value && typeof value === 'object') {
    for (const [key, v] of Object.entries(value)) {
      if (key === 'position') positions.push(v);
      else findPositions(v, positions);
    }
  }
  return positions;
}

const round = (v: number) => Math.round(v * 100) / 100;

const registry = new BenchmarkRegistry();
let changed = 0;

for (const file of await resolveTargets(targets)) {
  const text = await readFile(file, 'utf8');
  const json = JSON.parse(text);
  const levels = findLevels(json);

  const types = levels.flatMap((l) => l.nodes.map((n) => n.type));
  await registry.load(types.filter((t) => !t.startsWith('__internal/')) as NodeId[]);

  for (const level of levels) {
    const positions = layoutGraph(level.nodes, level.edges, (n) => registry.getNode(n.type));
    for (const node of level.nodes) {
      const [x, y] = positions.get(node.id)!;
      node.position = [round(x), round(y)];
    }
  }

  // write only the positions back, so the formatting of the rest of the file stays
  const positions = findPositions(json);
  const pattern = /"position"(\s*):(\s*)\[[^\]]*\]/g;
  if ((text.match(pattern) ?? []).length !== positions.length) {
    console.error(`${relative(process.cwd(), file)}: can not find the positions in the file`);
    process.exitCode = 1;
    continue;
  }
  let i = 0;
  const formatted = text.replace(pattern, () => {
    const position = positions[i++] as number[];
    return `"position": [${position.join(', ')}]`;
  });

  if (formatted === text) continue;
  changed++;
  if (options.check) {
    console.log(`${relative(process.cwd(), file)}: layout is not formatted`);
  } else {
    await writeFile(file, formatted);
    console.log(`${relative(process.cwd(), file)}: formatted`);
  }
}

if (options.check && changed > 0) process.exitCode = 1;
