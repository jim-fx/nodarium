import type { Graph, NodeDefinition } from '@nodarium/types';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { basename, resolve } from 'node:path';
// the package itself, so the bundle of the command line reads the templates of dist/templates
import { loadTemplate, templates, unlistedTemplates } from '@nodarium/runtime/templates';
import { decodeGraph, graphLinkData } from '../graph-link';
import { parseProjectFile } from '../project-file';
import { type GraphIssue, validateGraph } from '../validate-graph';

/** A graph as the command line got it, before it is parsed */
export type GraphSource = {
  /**
   * the name of the file without `.json` or `.nodarium.json`, the template id, the title of the
   * graph of a link (`link` without one), or `stdin`
   */
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

/** The title of a graph as the name of a file */
function fileName(title: unknown) {
  if (typeof title !== 'string') return undefined;
  return title.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || undefined;
}

/** A template id, the path of a graph or a project file, a link of the app, or `-` for stdin */
export async function readGraphSource(target: string): Promise<GraphSource> {
  if (target === '-') return { name: 'stdin', json: parseJson(await readStdin(), 'stdin') };
  const data = /^https?:\/\//.test(target) ? graphLinkData(target) : undefined;
  if (data !== undefined) {
    const json = await decodeGraph(data);
    const graph = writtenGraph(json) as { meta?: { title?: unknown } } | undefined;
    return { name: fileName(graph?.meta?.title) ?? 'link', json };
  }
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

/** What `--set` changes: settings of the run, and inputs of nodes */
export type GraphSets = {
  settings: Record<string, unknown>;
  props: { node: number; input: string; value: unknown }[];
};

/**
 * `resolution.curve=16` sets a setting, `4.length=0.3` the input length of node 4. A value is
 * read as JSON, a word stays a string. Throws for an entry without `=`.
 */
export function parseSets(sets: string[]): GraphSets {
  const settings: Record<string, unknown> = {};
  const props: GraphSets['props'] = [];
  for (const set of sets) {
    const at = set.indexOf('=');
    if (at < 1) throw new Error(`--set ${set}: write key=value`);
    const key = set.slice(0, at);
    const text = set.slice(at + 1);
    let value: unknown = text;
    try {
      value = JSON.parse(text);
    } catch {
      // a word stays a string, like the name of an option
    }
    const prop = key.match(/^(\d+)\.(.+)$/);
    if (prop) props.push({ node: Number(prop[1]), input: prop[2], value });
    else settings[key] = value;
  }
  return { settings, props };
}

/**
 * Sets the inputs of {@link parseSets} on the nodes of a project file or a bare graph as they
 * are written, before it is parsed. Throws for a node the graph doesn't have.
 */
export function applySets(json: unknown, props: GraphSets['props']) {
  const written = writtenGraph(json) as Graph | undefined;
  for (const { node, input, value } of props) {
    const found = written?.nodes?.find((n) => n.id === node);
    if (!found) throw new Error(`--set ${node}.${input}: the graph has no node ${node}`);
    found.props = { ...found.props, [input]: value as number };
  }
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
