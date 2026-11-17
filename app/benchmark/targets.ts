import {
  applySets,
  type GraphSets,
  parseProjectFile,
  readGraphSource
} from '@nodarium/runtime/node';
import type { Graph } from '@nodarium/types';
import { existsSync, readdirSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { type Scale, userPath } from './args.ts';

export const BENCH_TEMPLATES = join(import.meta.dirname, 'templates');
export const NODES_DIR = join(import.meta.dirname, '..', 'static', 'nodes');

export function benchTemplateNames() {
  return readdirSync(BENCH_TEMPLATES)
    .filter(file => file.endsWith('.json'))
    .map(file => file.slice(0, -'.json'.length));
}

/**
 * A template of benchmark/templates first, then a file, then a template id of the runtime:
 * `plant` is the bench's own, `./plant.json` a file, `fern` the runtime's
 */
export function resolveTarget(
  target: string,
  benchNames: string[],
  exists: (path: string) => boolean = existsSync,
  cwd?: string
): { kind: 'bench'; name: string } | { kind: 'source'; target: string } {
  if (benchNames.includes(target)) return { kind: 'bench', name: target };
  const path = userPath(target, cwd);
  return { kind: 'source', target: exists(path) ? path : target };
}

export type Target = { name: string; graph: Graph };

/** The graph of a target with the inputs of `--set` changed, see {@link resolveTarget} */
export async function loadTarget(
  target: string,
  props: GraphSets['props'] = []
): Promise<Target> {
  const resolved = resolveTarget(target, benchTemplateNames());
  if (resolved.kind === 'bench') {
    // as they are written, like CI has always run them
    const json = JSON.parse(await readFile(join(BENCH_TEMPLATES, target + '.json'), 'utf8'));
    applySets(json, props);
    return { name: target, graph: json as Graph };
  }
  const source = await readGraphSource(resolved.target);
  applySets(source.json, props);
  return { name: source.name, graph: parseProjectFile(source.json).graph };
}

const allNodes = (graph: Graph) => [
  ...graph.nodes,
  ...(graph.groups ?? []).flatMap(group => group.nodes)
];

/** A graph with a Myzel node takes seconds to build, not milliseconds */
export function isMushroom(graph: Graph) {
  return allNodes(graph).some(node => node.type.startsWith('max/myzel/'));
}

/**
 * Multiplies a number input of every node of a type, in the groups too. An input the node
 * doesn't set starts from `fallback`, the default of its definition. A whole number stays whole.
 * Returns how many nodes changed, per scale.
 */
export function scaleProps(
  graph: Graph,
  scales: Scale[],
  fallback: (type: string, prop: string) => unknown = () => undefined
) {
  const nodes = allNodes(graph);
  return scales.map(({ type, prop, factor }) => {
    let changed = 0;
    for (const node of nodes) {
      if (node.type !== type && node.type.split('/').pop() !== type) continue;
      const value = node.props?.[prop] ?? fallback(node.type, prop);
      if (typeof value !== 'number') continue;
      const scaled = value * factor;
      node.props = { ...node.props, [prop]: Number.isInteger(value) ? Math.round(scaled) : scaled };
      changed++;
    }
    return changed;
  });
}
