import type { Graph, NodeDefinition } from '@nodarium/types';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import { findNodes, formatNode, formatNodeList, nodesOfType } from '../catalog';
import { GRAPH_LINK_BASE } from '../graph-link';
import { listNodes } from '../list-nodes';
import { nodesDir } from '../nodes-dir';
import { placeNodes } from '../place-nodes';
import { FileNodeRegistry } from '../registry/file-node-registry';
import type { GraphIssue } from '../validate-graph';
import {
  checkReport,
  graphError,
  issueLines,
  linkGraph,
  printedGraph,
  renderGraph,
  runGraph,
  runSettings,
  stringifyGraph
} from './graph-commands';
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
  link <graph...> [--base url] prints a link that opens the graph in the app
  mcp                          starts an MCP server on stdio with these commands as tools

  <graph>        a template id (fern, porcini, ...), the path of a graph or of a
                 .nodarium.json project, a link of the app, or - for stdin
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
  --base url     the app a link opens, ${GRAPH_LINK_BASE} by default
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
  base: { type: 'string' },
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
  base?: string;
  help?: boolean;
};

class CliError extends Error {}

/** The name a target is reported under when it can't be read */
const targetName = (target: string) => (target === '-' ? 'stdin' : target);

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

  private printIssues(name: string, issues: GraphIssue[]) {
    for (const line of issueLines(name, issues)) console.error(line);
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
    const error = graphError(loaded);
    if (error) {
      // with --json the error goes to stdout, stderr still names it next to the issues
      if (invalid && this.options.json) console.error(`${name}: error ${invalid}`);
      this.fail(name, error, issues);
      return;
    }
    return { name, graph: loaded.graph! };
  }

  private settings(graph: Graph) {
    const time = this.options.time === undefined ? {} : { 'time.t': Number(this.options.time) };
    return runSettings(graph, { ...this.sets.settings, ...time }, Number(this.options.seed));
  }

  async check(targets: string[]) {
    for (const target of targets) {
      const loaded = await this.load(target);
      if (!loaded) continue;
      const { name, issues, invalid, ok } = loaded;
      if (!ok) this.exitCode = 1;
      if (this.options.json) this.print({ name, ok, issues, invalid });
      else this.print(checkReport(name, loaded));
    }
  }

  async run(targets: string[]) {
    for (const target of targets) {
      const prepared = await this.prepare(target);
      if (!prepared) continue;
      const { name } = prepared;
      if (this.options['print-graph']) {
        this.print(stringifyGraph(printedGraph(prepared.graph, this.sets.settings)));
        continue;
      }

      let ran;
      try {
        ran = await runGraph(this.registry, prepared.graph, this.settings(prepared.graph), {
          timings: this.options.timings
        });
      } catch (error) {
        this.fail(name, error instanceof Error ? error.message : String(error));
        continue;
      }
      const summary = { name, ...ran.summary };
      const empty = summary.min === undefined;
      if (this.options.json) {
        this.print(ran.nodes ? { ...summary, nodes: ran.nodes } : summary);
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
      if (ran.nodes) {
        for (const t of ran.nodes) {
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
      const nodeId = probe ? Number(probe[1]) : undefined;
      let rendered;
      try {
        rendered = await renderGraph(this.registry, prepared.graph, this.settings(prepared.graph), {
          size: Number(this.options.size),
          below: this.options.below,
          focus,
          node: probe ? { id: nodeId!, output: Number(probe[2] ?? 0) } : undefined
        });
      } catch (error) {
        this.fail(name, error instanceof Error ? error.message : String(error));
        continue;
      }
      const screenshot = join(out, `${name}${nodeId === undefined ? '' : `-node${nodeId}`}.png`);
      await writeFile(screenshot, rendered.png);
      const line = {
        name,
        ...(nodeId === undefined ? {} : { node: nodeId, type: rendered.type }),
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

  async link(targets: string[]) {
    const base = this.options.base ?? GRAPH_LINK_BASE;
    if (!/^https?:\/\/[^/]/.test(base)) {
      throw new CliError(`--base takes an http(s) address, not ${base}`);
    }
    for (const target of targets) {
      const prepared = await this.prepare(target);
      if (!prepared) continue;
      const { name } = prepared;
      const link = await linkGraph(prepared.graph, name, this.sets.settings, base);
      this.print(this.options.json ? { name, link, length: link.length } : link);
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

const MCP_SDK_MISSING = 'nodarium mcp needs the MCP SDK: npm install @modelcontextprotocol/sdk, '
  + 'or start it with npx -p @nodarium/runtime -p @modelcontextprotocol/sdk nodarium mcp';

/** Starts the server of `nodarium mcp`, which keeps running after this returns */
async function serveMcp(dir: string) {
  let mcp;
  try {
    // the SDK is an optional peer dependency, only this command loads it
    mcp = await import('../mcp/server');
  } catch (error) {
    const missing = (error as NodeJS.ErrnoException).code === 'ERR_MODULE_NOT_FOUND'
      && String((error as Error).message).includes('@modelcontextprotocol/sdk');
    if (!missing) throw error;
    console.error(MCP_SDK_MISSING);
    return 1;
  }
  await mcp.serveStdio(dir);
  return 0;
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

  if (command === 'mcp') return serveMcp(options.nodes ?? nodesDir);

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
      case 'link':
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
