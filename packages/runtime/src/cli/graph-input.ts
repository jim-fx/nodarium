import type { Graph, NodeDefinition } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { basename, resolve } from 'node:path';
// the package itself, so the bundle of the command line reads the templates of dist/templates
import { loadTemplate, templates, unlistedTemplates } from '@nodarium/runtime/templates';
import { parseProjectFile } from '../project-file';
import { type GraphIssue, validateGraph } from '../validate-graph';

/** A graph as the command line got it, before it is parsed */
export type GraphSource = {
  /** the name of the file without `.json` or `.nodarium.json`, the template id, or `stdin` */
  name: string;
  /** the file it was read from */
  file?: string;
  /** the parsed JSON: a project file or a bare graph */
  json: unknown;
};

async function readStdin() {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString('utf8');
}

function parseJson(text: string, from: string) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${from} is no JSON: ${error instanceof Error ? error.message : error}`);
  }
}

/** A template id, the path of a graph or a project file, or `-` for stdin */
export async function readGraphSource(target: string): Promise<GraphSource> {
  if (target === '-') return { name: 'stdin', json: parseJson(await readStdin(), 'stdin') };
  const file = resolve(target);
  if (existsSync(file)) {
    const name = basename(file).replace(/(\.nodarium)?\.json$/, '');
    return { name, file, json: parseJson(await readFile(file, 'utf8'), target) };
  }
  if ([...templates, ...unlistedTemplates].some((t) => t.id === target)) {
    return { name: target, json: structuredClone(await loadTemplate(target)) };
  }
  const ids = templates.map((t) => t.id).join(', ');
  throw new Error(`there is no file ${target} and no template of that id (templates: ${ids})`);
}

/** The graph as it is written in a project file or on its own */
export function writtenGraph(json: unknown): unknown {
  return json && typeof json === 'object' && 'format' in json
    ? (json as { graph?: unknown }).graph
    : json;
}

export type CheckedGraph = {
  /** the graph parsed and migrated, missing when the format check rejected it */
  graph?: Graph;
  /** what validateGraph finds in the graph as it is written */
  issues: GraphIssue[];
  /** why the format check rejected the file */
  invalid?: string;
  /** whether the graph can run: no error among the issues and a graph that passed */
  ok: boolean;
};

/**
 * Parses a project file or a bare graph and checks it with `validateGraph` as it is written. A
 * file the format check rejects has no graph but its issues all the same.
 */
export function checkGraph(json: unknown, definitions: Map<string, NodeDefinition>): CheckedGraph {
  const written = writtenGraph(json);
  const issues = written && typeof written === 'object'
    ? validateGraph(written as Graph, definitions)
    : [];
  const ok = !issues.some((issue) => issue.severity === 'error');
  try {
    return { graph: parseProjectFile(json).graph, issues, ok };
  } catch (error) {
    return { issues, invalid: error instanceof Error ? error.message : String(error), ok: false };
  }
}
