import { DEBUG_NODE, type Graph, type NodeRegistry } from '@nodarium/types';
import { expandGroups } from '../expand-groups';
import { graphLink } from '../graph-link';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { placeNodes } from '../place-nodes';
import {
  type RenderedSheet,
  type RenderOptions,
  renderSheetToPng,
  type RenderStats,
  renderToPng
} from '../render';
import { bounds, collectTriangles } from '../render/triangles';
import { decodeResult } from '../result/decode-result';
import { MemoryRuntimeExecutor } from '../runtime-executor';
import type { GraphIssue } from '../validate-graph';
import type { CheckedGraph } from './graph-input';
import { instancers, type InstancerSummary } from './probe';

// What `nodarium check/run/render/link` and the tools of `nodarium mcp` do with a graph, so both
// give the same answer for the same graph.

/** The outputs `--node` can draw, like the debug view of the app */
const DRAWN_TYPES = new Set(['path', 'geometry', 'points', 'mesh', 'pattern']);

/** Indented JSON with lists of numbers and strings, like positions and edges, on one line */
export function stringifyGraph(value: unknown, indent = ''): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value) ?? 'null';
  const inner = indent + '  ';
  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    if (value.every((item) => typeof item === 'number' || typeof item === 'string')) {
      return `[${value.map((item) => JSON.stringify(item)).join(', ')}]`;
    }
    return `[\n${value.map((item) => inner + stringifyGraph(item, inner)).join(',\n')}\n${indent}]`;
  }
  const entries = Object.entries(value).filter(([, item]) => item !== undefined);
  if (entries.length === 0) return '{}';
  const lines = entries.map(([key, item]) =>
    `${inner}${JSON.stringify(key)}: ${stringifyGraph(item, inner)}`
  );
  return `{\n${lines.join(',\n')}\n${indent}}`;
}

function round(v: number) {
  return Math.round(v * 10000) / 10000;
}

/** An issue per line: `<name>: <severity> <code>: <message>` */
export function issueLines(name: string, issues: GraphIssue[]) {
  return issues.map((issue) => `${name}: ${issue.severity} ${issue.code}: ${issue.message}`);
}

/** What `check` prints for a graph without --json */
export function checkReport(name: string, { issues, invalid }: CheckedGraph) {
  const lines = issueLines(name, issues);
  if (invalid) lines.push(`${name}: error ${invalid}`);
  if (!issues.length && !invalid) lines.push(`${name}: no issues`);
  return lines.join('\n');
}

/** Why a checked graph can't run, undefined when it can */
export function graphError({ ok, invalid, issues }: CheckedGraph) {
  if (ok) return undefined;
  return invalid ?? issues.find((issue) => issue.severity === 'error')!.message;
}

/** The settings of a run: those of the graph, `settings` on top, and the seed */
export function runSettings(graph: Graph, settings: Record<string, unknown>, seed: number) {
  // a fixed seed makes runs comparable
  return { ...graph.settings, ...settings, seed };
}

/** The graph as `run --print-graph` prints it: with a version and a position for every node */
export function printedGraph(graph: Graph, settings: Record<string, unknown>) {
  const { id, settings: own, ...rest } = placeNodes(graph);
  return { id, ...rest, version: CURRENT_GRAPH_VERSION, settings: { ...own, ...settings } };
}

export type RunSummary = {
  vertices: number;
  faces: number;
  instances: number;
  meshes: number;
  /** the bounds in meters, left out when the graph made nothing */
  min?: number[];
  max?: number[];
  height?: number;
  width?: number;
  ms: number;
  /** every instance node with how many instances it placed and their size in m */
  instancers?: InstancerSummary[];
};

export type NodeTiming = {
  id: number;
  type: string;
  /** the group instance the node runs in */
  group?: number;
  ms: number;
  /** of its outputs */
  bytes: number;
};

/**
 * Builds a graph and measures what came out. With `timings` the groups are expanded and every
 * node that ran is listed, slowest first. Throws when the graph fails.
 */
export async function runGraph(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>,
  { timings = false } = {}
): Promise<{ summary: RunSummary; nodes?: NodeTiming[] }> {
  const origins = new Map<number, number>();
  const run = timings ? expandGroups(graph, origins) : graph;
  const runtime = new MemoryRuntimeExecutor(registry);
  const started = performance.now();
  let ran;
  try {
    ran = await runtime.run(run, settings);
  } finally {
    runtime.releaseInstances();
  }
  const ms = performance.now() - started;
  const decoded = decodeResult(ran.result);
  const { triangles } = collectTriangles(ran.result);
  const { min, max } = bounds(triangles);
  const empty = triangles.length === 0;
  const summary = {
    vertices: decoded.vertexCount,
    faces: decoded.faceCount,
    instances: decoded.instances.reduce((sum, i) => sum + i.instanceCount, 0),
    meshes: decoded.meshes.length,
    min: empty ? undefined : min.map(round),
    max: empty ? undefined : max.map(round),
    height: empty ? undefined : round(max[1] - Math.min(min[1], 0)),
    width: empty ? undefined : round(Math.max(max[0] - min[0], max[2] - min[2])),
    ms: Math.round(ms * 10) / 10
  };
  const placed = await instancers(registry, graph, settings);
  if (placed.length) Object.assign(summary, { instancers: placed });
  if (!timings) return { summary };
  const types = new Map(run.nodes.map((node) => [node.id, node.type]));
  const nodes = Object.entries(ran.runTimings)
    .filter(([id]) => types.has(Number(id)))
    .map(([id, ms]) => ({
      id: Number(id),
      type: types.get(Number(id))!,
      group: origins.get(Number(id)),
      ms: Math.round(ms * 100) / 100,
      bytes: ran.outputBytes?.[Number(id)] ?? 0
    }))
    .sort((a, b) => b.ms - a.ms);
  return { summary, nodes };
}

export type RenderGraphOptions = Omit<RenderOptions, 'paths'> & {
  /** draws what comes out of this node instead of the graph's result */
  node?: { id: number; output?: number };
};

export type RenderedGraph = {
  png: Uint8Array;
  stats: RenderStats;
  /** the type of the output of `node` that was drawn */
  type?: string;
};

/** A copy of the graph with a debug node behind `node`, like the probe of the editor */
function probedGraph(graph: Graph, node?: RenderGraphOptions['node']) {
  if (!node) return { graph };
  const probed = graph.nodes.find((n) => n.id === node.id);
  if (!probed) {
    const ids = graph.nodes.map((n) => `${n.id} ${n.type.split('/').pop()}`).join(', ');
    throw new Error(`the graph has no node ${node.id}, it has the nodes ${ids}`);
  }
  const debugId = Math.max(0, ...graph.nodes.map((n) => n.id)) + 1;
  return {
    probed,
    graph: {
      ...graph,
      nodes: [...graph.nodes, { id: debugId, type: DEBUG_NODE, props: {} }],
      edges: [...graph.edges, [node.id, node.output ?? 0, debugId, 'input']]
    } as Graph
  };
}

/** What a run gives to draw: the result, or the output of `node` with its type */
async function drawable(
  runtime: MemoryRuntimeExecutor,
  graph: Graph,
  settings: Record<string, unknown>,
  node?: RenderGraphOptions['node']
) {
  const ran = await runtime.run(graph, settings);
  const entry = node && ran.debugData[node.id];
  if (node && !entry) throw new Error(`node ${node.id} gave no output`);
  const type = entry ? entry.type : undefined;
  // values like floats and vectors are no geometry, whatever their first word is
  const drawn = type === undefined || DRAWN_TYPES.has(type);
  return { data: entry ? entry.data : ran.result, type, drawn };
}

function nothingToDraw(probed: Graph['nodes'][number] | undefined, id?: number, type?: string) {
  return new Error(
    probed
      ? `${probed.type.split('/').pop()}#${id} gives a ${type}, there is nothing to draw`
      : 'the graph produced no geometry'
  );
}

/**
 * Builds a graph and draws front, side and top view of its result, or of the output of one of
 * its nodes. Throws when the graph fails or there is nothing to draw.
 */
export async function renderGraph(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>,
  { node, ...options }: RenderGraphOptions = {}
): Promise<RenderedGraph> {
  const probe = probedGraph(graph, node);
  const runtime = new MemoryRuntimeExecutor(registry);
  let built;
  try {
    built = await drawable(runtime, probe.graph, settings, node);
  } finally {
    runtime.releaseInstances();
  }
  const rendered = built.drawn
    && renderToPng(built.data, { ...options, paths: node !== undefined });
  if (!rendered) throw nothingToDraw(probe.probed, node?.id, built.type);
  return { ...rendered, type: built.type };
}

/** A cell of a sheet: the seed and the time it is built with */
export type SheetRun = { seed: number; time?: number };

export type RenderedGraphSheet = Omit<RenderedSheet, 'cells'> & {
  cells: (
    & SheetRun
    & Omit<RenderedSheet['cells'][number], 'stats'>
    & Partial<RenderStats>
    & { empty?: true }
  )[];
  type?: string;
};

/**
 * Builds a graph once per cell, with the seed and the time of that cell on `settings`, and draws
 * them all into one sheet at the same scale, `columns` cells in a row. A cell labelled
 * `s<seed> t<time>` that drew nothing stays empty. Throws when the graph fails or no cell drew
 * anything.
 */
export async function renderGraphSheet(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>,
  runs: SheetRun[],
  { node, columns, ...options }: RenderGraphOptions & { columns?: number }
): Promise<RenderedGraphSheet> {
  const probe = probedGraph(graph, node);
  const runtime = new MemoryRuntimeExecutor(registry);
  const cells = [];
  let type: string | undefined;
  try {
    for (const run of runs) {
      const time = run.time === undefined ? {} : { 'time.t': run.time };
      const built = await drawable(
        runtime,
        probe.graph,
        { ...settings, ...time, seed: run.seed },
        node
      );
      type = built.type;
      if (!built.drawn) throw nothingToDraw(probe.probed, node?.id, built.type);
      const label = `s${run.seed}${run.time === undefined ? '' : ` t${run.time}`}`;
      cells.push({ result: built.data, label });
    }
  } finally {
    runtime.releaseInstances();
  }
  const sheet = renderSheetToPng(cells, { ...options, columns, paths: node !== undefined });
  if (!sheet) throw nothingToDraw(probe.probed, node?.id, type);
  return {
    ...sheet,
    type,
    cells: sheet.cells.map(({ stats, ...at }, i) => ({
      ...runs[i],
      ...at,
      ...(stats ?? { empty: true as const })
    }))
  };
}

/** The link that opens a graph in the app, titled `name` when it has no title of its own */
export function linkGraph(
  graph: Graph,
  name: string,
  settings: Record<string, unknown>,
  base?: string
) {
  // the app gives the project an id of its own and the title of the graph
  const { version, meta, nodes, edges, groups } = graph;
  return graphLink({
    version,
    meta: { title: meta?.title ?? name, unit: meta?.unit },
    settings: { ...graph.settings, ...settings },
    nodes,
    edges,
    groups
  }, base);
}
