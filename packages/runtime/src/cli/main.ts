import { DEBUG_NODE, type Graph, type NodeDefinition } from '@nodarium/types';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import { findNodes, formatNode, formatNodeList, nodesOfType } from '../catalog';
import { expandGroups } from '../expand-groups';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { listNodes } from '../list-nodes';
import { nodesDir } from '../nodes-dir';
import { placeNodes } from '../place-nodes';
import { FileNodeRegistry } from '../registry/file-node-registry';
import { renderToPng } from '../render';
import { bounds, collectTriangles } from '../render/triangles';
import { decodeResult } from '../result/decode-result';
import { MemoryRuntimeExecutor } from '../runtime-executor';
import type { GraphIssue } from '../validate-graph';
import {
  applySets,
  checkGraph,
  type GraphSets,
  type GraphSource,
  parseSets,
  readGraphSource,
  writtenGraph
} from './graph-input';

export const USAGE = `Usage: nodarium <command> [options]

  nodes [name...] [--type t]   the nodes: a list, or the inputs of the named nodes
  check <graph...>             lists the mistakes in a graph, exits with 1 on an error
  run <graph...>               builds a graph and prints its size
      [--seed n] [--time t] [--set key=value]... [--timings] [--print-graph]
  render <graph...>            renders front, side and top view into a PNG
      [--out dir] [--size px] [--seed n] [--time t] [--set key=value]...
      [--focus x,z,r] [--below] [--node id[:output]]
  format <graph...> [--check]  places the nodes that have no position

  <graph>        a template id (fern, porcini, ...), the path of a graph or of a
                 .nodarium.json project, or - for stdin
  --json         prints JSON
  --nodes dir    the folder of the compiled nodes, the nodes of the package by default
  --seed n       the seed of the random nodes, 1 by default
  --time t       sets the setting time.t, from 0 to 1
  --set k=v      sets a setting (resolution.curve=16) or the input of a node (4.length=0.3)
  --timings      the ms and output bytes of every node
  --print-graph  prints the graph with the changes of --set instead of running it
  --out dir      where render writes <name>.png, ./screenshots by default
  --size px      the size of one view, 512 by default
  --focus x,z,r  only what lies within r (m) of x, z seen from above
  --below        adds a view from below
  --node id      renders the output of that node instead of the graph's
  --check        format only reports the graphs with nodes that have no position
`;

const OPTIONS = {
  json: { type: 'boolean' },
  nodes: { type: 'string' },
  type: { type: 'string' },
  seed: { type: 'string', default: '1' },
  time: { type: 'string' },
  set: { type: 'string', multiple: true },
  timings: { type: 'boolean' },
  'print-graph': { type: 'boolean' },
  out: { type: 'string' },
  size: { type: 'string', default: '512' },
  focus: { type: 'string' },
  below: { type: 'boolean' },
  node: { type: 'string' },
  check: { type: 'boolean' },
  help: { type: 'boolean', short: 'h' }
} as const;

type Options = {
  json?: boolean;
  nodes?: string;
  type?: string;
  seed: string;
  time?: string;
  set?: string[];
  timings?: boolean;
  'print-graph'?: boolean;
  out?: string;
  size: string;
  focus?: string;
  below?: boolean;
  node?: string;
  check?: boolean;
  help?: boolean;
};

class CliError extends Error {}

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

/** The name a target is reported under when it can't be read */
const targetName = (target: string) => (target === '-' ? 'stdin' : target);

function round(v: number) {
  return Math.round(v * 10000) / 10000;
}

function kilobytes(bytes: number) {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} kB`;
}

function numbers(text: string, count: number, flag: string) {
  const values = text.split(',').map(Number);
  if (values.length !== count || values.some((v) => !Number.isFinite(v))) {
    throw new CliError(`--${flag} takes ${count} numbers separated by commas, not ${text}`);
  }
  return values;
}

class Cli {
  private definitions?: Map<string, NodeDefinition>;
  private readonly registry: FileNodeRegistry;
  private readonly sets: GraphSets;
  exitCode = 0;

  constructor(private readonly options: Options, private readonly dir: string) {
    this.registry = new FileNodeRegistry(dir);
    try {
      this.sets = parseSets(options.set ?? []);
    } catch (error) {
      throw new CliError((error as Error).message);
    }
  }

  private async allNodes() {
    if (!this.definitions) {
      const nodes = await listNodes(this.dir, this.registry).catch(() => {
        throw new CliError(
          `No nodes in ${this.dir}: build them with \`pnpm build:nodes\` or pass --nodes <dir>`
        );
      });
      this.definitions = new Map(nodes.map((node) => [node.id, node]));
    }
    return this.definitions;
  }

  private print(value: unknown) {
    console.log(typeof value === 'string' ? value : JSON.stringify(value));
  }

  /** One graph that failed, the others still run */
  private fail(name: string, error: string, issues?: GraphIssue[]) {
    this.exitCode = 1;
    if (this.options.json) this.print({ name, error, issues });
    else console.error(`${name}: error ${error}`);
  }

  private printIssues(name: string, issues: GraphIssue[], to = console.error) {
    for (const issue of issues) to(`${name}: ${issue.severity} ${issue.code}: ${issue.message}`);
  }

  async nodes(names: string[]) {
    let nodes = [...(await this.allNodes()).values()];
    if (this.options.type) nodes = nodesOfType(nodes, this.options.type);
    if (names.length) {
      nodes = names.flatMap((name) => {
        const found = findNodes(nodes, name);
        if (!found.length) {
          throw new CliError(`there is no node ${name}, \`nodarium nodes\` lists them`);
        }
        return found;
      });
    }
    if (this.options.json) this.print(JSON.stringify(nodes, null, 2));
    else if (names.length) this.print(nodes.map((node) => formatNode(node)).join('\n\n'));
    else this.print(formatNodeList(nodes));
  }

  /** The graph of a target with the nodes changed by --set and its issues */
  private async load(target: string) {
    let source: GraphSource;
    try {
      source = await readGraphSource(target);
    } catch (error) {
      this.fail(targetName(target), error instanceof Error ? error.message : String(error));
      return;
    }
    const { name } = source;
    try {
      applySets(source.json, this.sets.props);
    } catch (error) {
      this.fail(name, (error as Error).message);
      return;
    }
    return { name, source, ...checkGraph(source.json, await this.allNodes()) };
  }

  /**
   * The graph of a target with the nodes changed by --set, its issues printed to stderr.
   * Undefined when it can't run.
   */
  private async prepare(target: string) {
    const loaded = await this.load(target);
    if (!loaded) return;
    const { name, issues, invalid } = loaded;
    this.printIssues(name, issues);
    if (!loaded.ok) {
      // with --json the error goes to stdout, stderr still names it next to the issues
      if (invalid && this.options.json) console.error(`${name}: error ${invalid}`);
      const error = invalid ?? issues.find((issue) => issue.severity === 'error')!.message;
      this.fail(name, error, issues);
      return;
    }
    return { name, graph: loaded.graph! };
  }

  private settings(graph: Graph) {
    const time = this.options.time === undefined ? {} : { 'time.t': Number(this.options.time) };
    // a fixed seed makes runs comparable
    return { ...graph.settings, ...this.sets.settings, ...time, seed: Number(this.options.seed) };
  }

  async check(targets: string[]) {
    for (const target of targets) {
      const loaded = await this.load(target);
      if (!loaded) continue;
      const { name, issues, invalid, ok } = loaded;
      if (!ok) this.exitCode = 1;
      if (this.options.json) this.print({ name, ok, issues, invalid });
      else {
        this.printIssues(name, issues, console.log);
        if (invalid) this.print(`${name}: error ${invalid}`);
        if (!issues.length && !invalid) this.print(`${name}: no issues`);
      }
    }
  }

  async run(targets: string[]) {
    for (const target of targets) {
      const prepared = await this.prepare(target);
      if (!prepared) continue;
      const { name } = prepared;
      if (this.options['print-graph']) {
        const { id, settings, ...rest } = placeNodes(prepared.graph);
        const printed = {
          id,
          ...rest,
          version: CURRENT_GRAPH_VERSION,
          settings: { ...settings, ...this.sets.settings }
        };
        this.print(stringifyGraph(printed));
        continue;
      }

      const origins = new Map<number, number>();
      const graph = this.options.timings ? expandGroups(prepared.graph, origins) : prepared.graph;
      const runtime = new MemoryRuntimeExecutor(this.registry);
      const started = performance.now();
      let ran;
      try {
        ran = await runtime.run(graph, this.settings(graph));
      } catch (error) {
        this.fail(name, error instanceof Error ? error.message : String(error));
        continue;
      } finally {
        runtime.releaseInstances();
      }
      const ms = performance.now() - started;
      const decoded = decodeResult(ran.result);
      const { triangles } = collectTriangles(ran.result);
      const { min, max } = bounds(triangles);
      const empty = triangles.length === 0;
      const summary = {
        name,
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
      const types = new Map(graph.nodes.map((node) => [node.id, node.type]));
      const timings = Object.entries(ran.runTimings)
        .filter(([id]) => types.has(Number(id)))
        .map(([id, ms]) => ({
          id: Number(id),
          type: types.get(Number(id))!,
          group: origins.get(Number(id)),
          ms: Math.round(ms * 100) / 100,
          bytes: ran.outputBytes?.[Number(id)] ?? 0
        }))
        .sort((a, b) => b.ms - a.ms);

      if (this.options.json) {
        this.print(this.options.timings ? { ...summary, nodes: timings } : summary);
        continue;
      }
      const parts = [`${summary.vertices} vertices`, `${summary.faces} faces`];
      if (summary.instances) parts.push(`${summary.instances} instances`);
      if (summary.meshes) parts.push(`${summary.meshes} meshes`);
      this.print(`${name}: ${parts.join(', ')}`);
      if (!empty) {
        this.print(
          `  from [${summary.min!.join(', ')}] to [${summary.max!.join(', ')}] m, `
            + `${summary.height} m high, ${summary.width} m wide`
        );
      }
      this.print(`  built in ${summary.ms} ms`);
      if (this.options.timings) {
        for (const t of timings) {
          const node = `${t.type.split('/').pop()}#${t.id}${t.group ? ` in group#${t.group}` : ''}`;
          this.print(
            `  ${node.padEnd(36)} ${t.ms.toFixed(2).padStart(9)} ms ${
              kilobytes(t.bytes).padStart(10)
            }`
          );
        }
      }
    }
  }

  async render(targets: string[]) {
    const out = this.options.out ?? join(process.cwd(), 'screenshots');
    const focus = this.options.focus
      ? (numbers(this.options.focus, 3, 'focus') as [number, number, number])
      : undefined;
    const probe = this.options.node?.match(/^(\d+)(?::(\d+))?$/);
    if (this.options.node && !probe) throw new CliError(`--node takes a node id, like 4 or 4:1`);
    await mkdir(out, { recursive: true });

    for (const target of targets) {
      const prepared = await this.prepare(target);
      if (!prepared) continue;
      const { name } = prepared;
      let graph = prepared.graph;
      const nodeId = probe ? Number(probe[1]) : undefined;
      const probed = nodeId === undefined ? undefined : graph.nodes.find((n) => n.id === nodeId);
      if (nodeId !== undefined) {
        if (!probed) {
          const ids = graph.nodes.map((n) => `${n.id} ${n.type.split('/').pop()}`).join(', ');
          this.fail(name, `the graph has no node ${nodeId}, it has the nodes ${ids}`);
          continue;
        }
        // a debug node behind the node, in a copy of the graph, like the probe of the editor
        const debugId = Math.max(0, ...graph.nodes.map((n) => n.id)) + 1;
        graph = {
          ...graph,
          nodes: [...graph.nodes, { id: debugId, type: DEBUG_NODE, props: {} }],
          edges: [...graph.edges, [nodeId, Number(probe![2] ?? 0), debugId, 'input']]
        };
      }

      const runtime = new MemoryRuntimeExecutor(this.registry);
      let data: Int32Array;
      let shown: string | undefined;
      try {
        const ran = await runtime.run(graph, this.settings(graph));
        const entry = nodeId === undefined ? undefined : ran.debugData[nodeId];
        if (nodeId !== undefined && !entry) throw new Error(`node ${nodeId} gave no output`);
        data = entry?.data ?? ran.result;
        shown = entry?.type;
      } catch (error) {
        this.fail(name, error instanceof Error ? error.message : String(error));
        continue;
      } finally {
        runtime.releaseInstances();
      }

      // values like floats and vectors are no geometry, whatever their first word is
      const drawn = shown === undefined || DRAWN_TYPES.has(shown);
      const rendered = drawn && renderToPng(data, {
        size: Number(this.options.size),
        below: this.options.below,
        focus,
        paths: nodeId !== undefined
      });
      if (!rendered) {
        this.fail(
          name,
          probed
            ? `${probed.type.split('/').pop()}#${nodeId} gives a ${shown}, there is nothing to draw`
            : 'the graph produced no geometry'
        );
        continue;
      }
      const screenshot = join(out, `${name}${nodeId === undefined ? '' : `-node${nodeId}`}.png`);
      await writeFile(screenshot, rendered.png);
      const line = {
        name,
        ...(nodeId === undefined ? {} : { node: nodeId, type: shown }),
        screenshot,
        ...rendered.stats
      };
      if (this.options.json) this.print(line);
      else {
        const { stats } = rendered;
        const parts = [`${stats.triangles} triangles`];
        if (stats.instances) parts.push(`${stats.instances} instances`);
        this.print(
          `${name}: ${screenshot}, ${
            parts.join(', ')
          }, ${stats.height} m high, ${stats.width} m wide`
        );
      }
    }
  }

  async format(targets: string[]) {
    const definitions = await this.allNodes();
    // a header, a row per input that is shown, roughly as the editor draws a node
    const height = (node: { type: string }) => {
      const inputs = Object.values(definitions.get(node.type)?.inputs ?? {});
      const shown = inputs.filter((i) =>
        !i.hidden && !i.setting && i.type !== 'seed' && i.type !== 'creator'
      );
      return 10 + 10 * shown.length;
    };
    for (const target of targets) {
      let source: GraphSource;
      try {
        source = await readGraphSource(target);
      } catch (error) {
        this.fail(targetName(target), error instanceof Error ? error.message : String(error));
        continue;
      }
      const written = writtenGraph(source.json) as Graph;
      if (!written || !Array.isArray(written.nodes)) {
        this.fail(source.name, 'no graph in it, a graph has nodes and edges');
        continue;
      }
      const levels = [written, ...(written.groups ?? [])];
      const unplaced = levels.flatMap((level) =>
        level.nodes.filter((n) => !Array.isArray(n.position))
      );
      if (this.options.check) {
        if (unplaced.length) {
          this.exitCode = 1;
          this.print(`${source.name}: ${unplaced.length} nodes have no position`);
        }
        continue;
      }
      const placed = placeNodes(
        { ...written, edges: written.edges ?? [], groups: written.groups ?? [] },
        { height }
      );
      const graph = { ...written, nodes: placed.nodes, groups: written.groups && placed.groups };
      const json = written === source.json ? graph : { ...(source.json as object), graph };
      const text = stringifyGraph(json) + '\n';
      if (source.file) {
        if (unplaced.length) await writeFile(source.file, text);
        this.print(`${source.name}: placed ${unplaced.length} nodes`);
      } else {
        this.print(text.trimEnd());
      }
    }
  }
}

/** Runs the command line `args` (without `node` and the script), returns the exit code */
export async function main(args: string[]): Promise<number> {
  let parsed;
  try {
    parsed = parseArgs({ args, options: OPTIONS, allowPositionals: true });
  } catch (error) {
    console.error(`${error instanceof Error ? error.message : error}\n\n${USAGE}`);
    return 1;
  }
  const options = parsed.values as Options;
  const [command, ...rest] = parsed.positionals;
  if (options.help) {
    console.log(USAGE);
    return 0;
  }
  if (!command) {
    console.error(USAGE);
    return 1;
  }

  const cli = new Cli(options, options.nodes ?? nodesDir);
  // the runtime logs the migrations of a graph, a graph without a version gets a warning instead
  const info = console.info;
  console.info = () => {};
  try {
    switch (command) {
      case 'nodes':
        await cli.nodes(rest);
        break;
      case 'check':
      case 'run':
      case 'render':
      case 'format':
        if (rest.length === 0) throw new CliError(`${command} takes a graph\n\n${USAGE}`);
        await cli[command](rest);
        break;
      default:
        throw new CliError(`there is no command ${command}\n\n${USAGE}`);
    }
  } catch (error) {
    if (!(error instanceof CliError)) throw error;
    console.error(error.message);
    return 1;
  } finally {
    console.info = info;
  }
  return cli.exitCode;
}
