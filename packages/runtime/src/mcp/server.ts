import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import type { NodeDefinition } from '@nodarium/types';
import { z } from 'zod';
import { version } from '../../package.json';
import { findNodes, formatNode, formatNodeList, nodesOfType, searchNodes } from '../catalog';
import {
  checkReport,
  graphError,
  issueLines,
  linkGraph,
  printedGraph,
  renderGraph,
  renderGraphSheet,
  runGraph,
  runSettings,
  stringifyGraph
} from '../cli/graph-commands';
import { checkGraph, graphSourceOf } from '../cli/graph-input';
import { probeGraph, probeLines } from '../cli/probe';
import { docList, docText, templateList } from '../docs';
import { MAX_GRAPH_LINK_BYTES } from '../graph-link';
import { listNodes } from '../list-nodes';
import { nodesDir } from '../nodes-dir';
import { FileNodeRegistry } from '../registry/file-node-registry';
import { RENDER_VIEWS, type RenderView } from '../render';

/** The largest view `render_graph` draws, in pixels; four views of it are 4096 × 1024 */
export const MAX_RENDER_SIZE = 1024;

/** The cells of a sheet of `render_graph` */
export const MAX_SHEET_CELLS = 16;

/** The pixels of a sheet of `render_graph`, 4096 × 4096 */
const MAX_SHEET_PIXELS = 16_777_216;

/** The views of `render_graph`, next to each other in one image */
const VIEWS = { 'front-side-top': false, 'front-side-top-below': true } as const;

/** Lines joined into one paragraph, a model reads no line breaks into it */
const paragraph = (...lines: string[]) => lines.join(' ');

const SYSTEMS_TEXT = paragraph(
  'The nodes come in three systems, the second part of their id, and all of them work together.',
  'Core (max/core/...) has the value nodes every graph has: float, math, random, curve, alpha,',
  'time, mix_color and the others give and shape the values of inputs. Plantarium',
  '(max/plantarium/...) makes plants from paths: stems, branches, points, leaves and instances.',
  'Myzel (max/myzel/...) grows mushrooms on those paths: caps, strands, gills and veils, and',
  'hyphae_to_mesh turns the strands into a surface. Every graph ends in the one output,',
  'max/core/output, which takes only geometry: paths reach it through a max/plantarium/to_geometry,',
  'geometry and instances pass through that too. There is one set_color, max/plantarium/set_color:',
  'it colors paths and geometry, and the colors it gives the strands of a mushroom color the',
  'surface of hyphae_to_mesh. One join, max/plantarium/join, joins the paths and the geometry of',
  'both.'
);

const INSTRUCTIONS = paragraph(
  'Nodarium makes 3D plants and mushrooms from graphs of nodes. A graph is JSON: nodes with',
  'props, and edges between them. The tools work on graphs you pass as JSON, nothing is saved.',
  'The order that works: list_templates and get_template for a graph to start from, list_nodes',
  'and get_node for the nodes you use, check_graph before run_graph, render_graph to see it,',
  'probe_node to see the values or the sizes one node gives, and',
  'graph_link for a link that opens it in the editor. The prompt write_graph explains the format,',
  'get_docs gives the guides to the plant and mushroom nodes.',
  SYSTEMS_TEXT
);

const GRAPH = z.union([z.string(), z.record(z.string(), z.unknown())]).describe(
  'The graph: a JSON object with "nodes" and "edges" (a bare graph or a .nodarium.json project '
    + 'file), the id of a template like "fern", or a link of the app with #graph=. At most 1 MB '
    + 'of JSON.'
);
const SEED = z.number().int().optional().describe(
  'The seed of the random nodes, 1 by default. Another seed gives another plant of the same kind.'
);
const SETTINGS = z.record(z.string(), z.union([z.number(), z.string(), z.boolean()])).optional()
  .describe(
    'Settings for this run on top of those of the graph, like {"resolution.curve": 8} or '
      + '{"time.t": 0.5} for the growth of a mushroom.'
  );

const READ_ONLY = { readOnlyHint: true, idempotentHint: true, openWorldHint: false } as const;

function text(value: string): CallToolResult['content'][number] {
  return { type: 'text', text: value };
}

/** Runs a tool, a thrown error becomes an answer with `isError` the model reads to fix its call */
async function answer(run: () => Promise<CallToolResult>): Promise<CallToolResult> {
  try {
    return await run();
  } catch (error) {
    return {
      isError: true,
      content: [text(error instanceof Error ? error.message : String(error))]
    };
  }
}

function sizeOf(graph: unknown) {
  return new TextEncoder().encode(typeof graph === 'string' ? graph : JSON.stringify(graph)).length;
}

/** The prompt `write_graph`: how the tools fit together, then docs/GRAPHS.md as it is */
export function graphPrompt(request?: string) {
  const parts = [
    paragraph(
      'You write a graph for Nodarium, a node editor for procedural 3D plants and mushrooms, with',
      'the tools of this server. Start from the template closest to what is asked (list_templates,',
      'get_template), look up the nodes you change or add (list_nodes, then get_node for each),',
      'call check_graph and fix every error before run_graph or render_graph, look at the picture',
      'of render_graph, and give the link of graph_link at the end so the graph opens in the editor.'
    ),
    SYSTEMS_TEXT,
    paragraph(
      'The guide below names the commands of the `nodarium` command line. Here they are tools:',
      '`nodes` is list_nodes and get_node, `check` is check_graph, `run` is run_graph, `render` is',
      'render_graph, `probe` is probe_node, `link` is graph_link and `run <template> --print-graph`',
      'is get_template. A',
      '--set of an input is a change of the graph you pass, a --set of a setting goes into',
      '"settings".'
    ),
    docText('graphs')!.trim()
  ];
  if (request?.trim()) parts.push(`What to make: ${request.trim()}`);
  return parts.join('\n\n');
}

/** The MCP server of `nodarium mcp`, with the nodes of `dir` (the package's by default) */
export function createServer(dir = nodesDir): McpServer {
  const registry = new FileNodeRegistry(dir);
  let definitions: Promise<Map<string, NodeDefinition>> | undefined;
  const allNodes = () =>
    definitions ??= listNodes(dir, registry).then(
      (nodes) => new Map(nodes.map((node) => [node.id, node])),
      () => {
        definitions = undefined;
        throw new Error(`No nodes in ${dir}`);
      }
    );

  async function load(graph: unknown) {
    const bytes = sizeOf(graph);
    if (bytes > MAX_GRAPH_LINK_BYTES) {
      throw new Error(
        `the graph has ${Math.round(bytes / 1024)} KB of JSON, a graph may have ${
          MAX_GRAPH_LINK_BYTES / 1024
        } KB`
      );
    }
    const source = await graphSourceOf(graph);
    return { name: source.name, ...checkGraph(source.json, await allNodes()) };
  }

  /** A graph that can run, with its warnings; throws with all its issues when it can't */
  async function prepare(input: unknown) {
    const loaded = await load(input);
    const error = graphError(loaded);
    if (error) {
      throw new Error(
        `${loaded.name} has errors, nothing ran. Fix them and try again:\n${
          checkReport(loaded.name, loaded)
        }`
      );
    }
    const warnings = issueLines(loaded.name, loaded.issues);
    return { name: loaded.name, graph: loaded.graph!, warnings: warnings.map(text) };
  }

  const server = new McpServer(
    { name: 'nodarium', title: 'Nodarium', version },
    { instructions: INSTRUCTIONS }
  );

  server.registerTool('list_nodes', {
    title: 'List the nodes',
    description: 'Lists the nodes a graph can use under a heading per system, one entry per '
      + 'node: "id (types of the data it takes) -> type it gives", then the first sentence of '
      + 'what it does. Core nodes (max/core/...) give and shape values, like float, math, random, curve '
      + 'and time, and max/core/output, the end of every graph, takes only geometry. Plantarium '
      + 'nodes (max/plantarium/...) make plants, a to_geometry turns their paths into the geometry '
      + 'of the output, Myzel nodes (max/myzel/...) make mushrooms. Call get_node for the inputs '
      + 'of the nodes you use.',
    inputSchema: {
      query: z.string().optional().describe(
        'Only the nodes whose id or description contains this word, like "leaf" or "cap"'
      ),
      type: z.string().optional().describe(
        'Only the nodes that take or give data of this type, like "path", "geometry" or "pattern"'
      )
    },
    annotations: READ_ONLY
  }, ({ query, type }) =>
    answer(async () => {
      let nodes = [...(await allNodes()).values()];
      if (type) nodes = nodesOfType(nodes, type);
      if (query) nodes = searchNodes(nodes, query);
      return { content: [text(nodes.length ? formatNodeList(nodes) : 'No node matches.')] };
    }));

  server.registerTool('get_node', {
    title: 'Show a node',
    description: 'The full definition of a node: what it does, and every input with its type, '
      + 'default, range, unit and description. The input names are the keys of "props" and the '
      + 'last element of an edge.',
    inputSchema: {
      id: z.string().describe(
        'The id of a node (max/plantarium/stem) or its name alone (stem); several separated by '
          + 'spaces'
      )
    },
    annotations: READ_ONLY
  }, ({ id }) =>
    answer(async () => {
      const nodes = [...(await allNodes()).values()];
      const found = id.split(/[\s,]+/).filter(Boolean).flatMap((name) => {
        const named = findNodes(nodes, name);
        if (!named.length) throw new Error(`there is no node ${name}, list_nodes lists them`);
        return named;
      });
      return { content: [text(found.map((node) => formatNode(node)).join('\n\n'))] };
    }));

  server.registerTool('list_templates', {
    title: 'List the templates',
    description: 'The plants and mushrooms of the app, as "id: name (category, unit)", below it '
      + 'the species a plant models and a line on how the template is built. Every template is a '
      + 'graph to start from, get_template gives it. The unit is the one the app shows lengths '
      + 'in, the graph is in meters either way.',
    annotations: READ_ONLY
  }, () => answer(async () => ({ content: [text(templateList())] })));

  server.registerTool('get_template', {
    title: 'Get a template',
    description: 'The graph of a template as JSON, with a version and a position for every node. '
      + 'Change it and pass it to the other tools, or pass the template id itself.',
    inputSchema: { id: z.string().describe('The id of a template, like "fern" or "porcini"') },
    annotations: READ_ONLY
  }, ({ id }) =>
    answer(async () => {
      const { graph } = await prepare(id);
      return { content: [text(stringifyGraph(printedGraph(graph, {})))] };
    }));

  server.registerTool('check_graph', {
    title: 'Check a graph',
    description: 'Finds every mistake in a graph without running it: unknown nodes and inputs, '
      + "edges between types that don't fit, cycles, values far out of range. One issue per "
      + 'line, "<name>: <error|warning> <code>: <message>", with the fix where there is one, '
      + 'or "<name>: no issues". A graph with an error doesn\'t run.',
    inputSchema: { graph: GRAPH },
    annotations: READ_ONLY
  }, ({ graph }) =>
    answer(async () => {
      const loaded = await load(graph);
      return { content: [text(checkReport(loaded.name, loaded))] };
    }));

  server.registerTool('run_graph', {
    title: 'Run a graph',
    description: 'Builds the 3D model of a graph and answers with JSON: its vertices, faces, '
      + 'instances and geometries, its bounds in meters (min, max, height, width), the ms it took, '
      + '"instancers", how many instances every instance node placed and how long and wide they '
      + 'are in meters, and "nodes", the ms and output bytes of every node, slowest first. A '
      + 'mushroom takes a '
      + 'few seconds. A graph with errors comes back as an error with all its issues.',
    inputSchema: { graph: GRAPH, seed: SEED, settings: SETTINGS },
    annotations: READ_ONLY
  }, ({ graph, seed, settings }) =>
    answer(async () => {
      const prepared = await prepare(graph);
      const ran = await runGraph(
        registry,
        prepared.graph,
        runSettings(prepared.graph, settings ?? {}, seed ?? 1),
        { timings: true }
      );
      const summary = { name: prepared.name, ...ran.summary, nodes: ran.nodes };
      return { content: [text(JSON.stringify(summary)), ...prepared.warnings] };
    }));

  server.registerTool('probe_node', {
    title: 'Probe a node',
    description: 'What comes out of one node of a graph, to find out why a part is too large, '
      + 'missing or the same everywhere. For a float or integer output (random, curve, math, '
      + 'path_info, ...) every value the nodes read from it, by the node and input its edge goes '
      + 'into and the node that read it: how many, on how many paths, the smallest, largest and '
      + 'mean. For a path, geometry or points output the paths by depth with their lengths in '
      + 'meters, the instances with their length and width in meters, and the geometry with its '
      + 'extents. A graph with errors comes back as an error with all its issues.',
    inputSchema: {
      graph: GRAPH,
      node: z.number().int().describe('The id of the node of the graph to look at'),
      seed: SEED,
      settings: SETTINGS
    },
    annotations: READ_ONLY
  }, ({ graph, node, seed, settings }) =>
    answer(async () => {
      const prepared = await prepare(graph);
      const report = await probeGraph(
        registry,
        prepared.graph,
        runSettings(prepared.graph, settings ?? {}, seed ?? 1),
        node
      );
      return {
        content: [text(probeLines(prepared.name, report).join('\n')), ...prepared.warnings]
      };
    }));

  server.registerTool(
    'render_graph',
    {
      title: 'Render a graph',
      description: 'Builds a graph and draws it: one PNG with a view from the front, from the '
        + 'side and from above next to each other (and from below with views '
        + '"front-side-top-below"), all at the same scale, or the views of a list like ["front"]. '
        + 'Stems are brown, leaves green, geometry in its colors. Answers with the image and '
        + 'JSON of what it shows: triangles, instances, leaf area in m² and the bounds in meters. '
        + 'With "node" it draws what comes out of that node instead, paths as tubes and points as '
        + 'dots. With "seeds" or "times" it draws a sheet: the graph once per seed and time, a row '
        + 'per time and a column per seed, all at the same scale, each cell labelled "s<seed> '
        + 't<time>", and the JSON lists the cells with their seed, time, place and numbers.',
      inputSchema: {
        graph: GRAPH,
        seed: SEED,
        settings: SETTINGS,
        node: z.number().int().optional().describe(
          'The id of a node of the graph: draws its output instead of the result of the graph'
        ),
        output: z.number().int().min(0).optional().describe(
          'Which output of that node, 0 by default'
        ),
        views: z.union([
          z.enum(Object.keys(VIEWS) as [keyof typeof VIEWS]),
          z.array(z.enum(RENDER_VIEWS as [RenderView, ...RenderView[]])).min(1).max(4)
        ]).optional().describe(
          'Which views, "front-side-top" by default, or a list of "front", "side", "top" and '
            + '"below" in the order to draw them'
        ),
        size: z.number().int().min(64).max(MAX_RENDER_SIZE).optional().describe(
          `The size of one view in pixels, 512 by default and 256 in a sheet, at most ${MAX_RENDER_SIZE}`
        ),
        focus: z.tuple([z.number(), z.number(), z.number().positive()]).optional().describe(
          'A close-up from above: [x, z, r] draws only what lies within r meters of x, z'
        ),
        seeds: z.array(z.number().int()).min(1).max(MAX_SHEET_CELLS).optional().describe(
          'A sheet with a column per seed, like [1, 2, 3, 4], to see how much the seeds differ'
        ),
        times: z.array(z.number()).min(1).max(MAX_SHEET_CELLS).optional().describe(
          'A sheet with the graph at every time.t, like [0.25, 0.5, 1], to see it grow: a row of '
            + 'times, or a row per time with seeds'
        )
      },
      annotations: READ_ONLY
    },
    ({ graph, seed, settings, node, output, views, size, focus, seeds, times }) =>
      answer(async () => {
        const prepared = await prepare(graph);
        const shown = typeof views === 'string' || views === undefined
          ? { below: VIEWS[views ?? 'front-side-top'] }
          : { views };
        const options = {
          ...shown,
          focus,
          node: node === undefined ? undefined : { id: node, output }
        };
        const base = runSettings(prepared.graph, settings ?? {}, seed ?? 1);
        let png: Uint8Array;
        let about: object;
        if (seeds || times) {
          const seedList = seeds ?? [seed ?? 1];
          const timeList = times ?? [undefined];
          const cells = seedList.length * timeList.length;
          if (cells > MAX_SHEET_CELLS) {
            throw new Error(`a sheet has at most ${MAX_SHEET_CELLS} cells, not ${cells}`);
          }
          const viewCount = Array.isArray(views)
            ? views.length
            : views === 'front-side-top-below'
            ? 4
            : 3;
          const pixels = cells * viewCount * (size ?? 256) ** 2;
          if (pixels > MAX_SHEET_PIXELS) {
            throw new Error(
              `the sheet would have ${Math.round(pixels / 1e6)} million pixels, at most ${
                MAX_SHEET_PIXELS / 1e6
              } million: draw fewer views or cells, or a smaller size`
            );
          }
          const rendered = await renderGraphSheet(
            registry,
            prepared.graph,
            base,
            timeList.flatMap((time) => seedList.map((s) => ({ seed: s, time }))),
            { ...options, size: size ?? 256, columns: seeds ? seedList.length : timeList.length }
          );
          const { png: image, type, ...sheet } = rendered;
          png = image;
          about = { name: prepared.name, ...(node === undefined ? {} : { node, type }), ...sheet };
        } else {
          const rendered = await renderGraph(registry, prepared.graph, base, {
            ...options,
            size: size ?? 512
          });
          png = rendered.png;
          about = {
            name: prepared.name,
            ...(node === undefined ? {} : { node, type: rendered.type }),
            ...rendered.stats
          };
        }
        return {
          content: [
            { type: 'image', data: Buffer.from(png).toString('base64'), mimeType: 'image/png' },
            text(JSON.stringify(about)),
            ...prepared.warnings
          ]
        };
      })
  );

  server.registerTool('graph_link', {
    title: 'Link to a graph',
    description: 'A link that opens the graph in the Nodarium editor in a browser, as a new '
      + 'project. The graph is packed into the link (#graph=...), nothing is uploaded. Give it to '
      + 'the person you work for so they can look at the model in 3D and change it.',
    inputSchema: { graph: GRAPH },
    annotations: READ_ONLY
  }, ({ graph }) =>
    answer(async () => {
      const prepared = await prepare(graph);
      return { content: [text(await linkGraph(prepared.graph, prepared.name, {}))] };
    }));

  server.registerTool('get_docs', {
    title: 'Read the docs',
    description: 'The guides of Nodarium as markdown: "graphs" (how to write a graph), '
      + '"plant-templates" (what the plant nodes can express and how every plant template is '
      + 'built), "core" (the value nodes every graph has), "plantarium" (the data of the plant '
      + 'nodes), "myzel" (the mushroom nodes and their groups), "templates" (the list_templates '
      + 'list). Without a topic, the list of '
      + 'topics. The guides name the commands of the `nodarium` command line: `nodes` is '
      + 'list_nodes and get_node, `check` check_graph, `run` run_graph, `render` render_graph, '
      + '`probe` probe_node, `link` graph_link.',
    inputSchema: {
      topic: z.string().optional().describe('A topic like "plant-templates", or a file name')
    },
    annotations: READ_ONLY
  }, ({ topic }) =>
    answer(async () => {
      if (!topic) return { content: [text(docList())] };
      const doc = docText(topic);
      if (doc === undefined) {
        throw new Error(`there is no doc ${topic}, the topics are:\n${docList()}`);
      }
      return { content: [text(doc)] };
    }));

  server.registerPrompt('write_graph', {
    title: 'Write a Nodarium graph',
    description: 'How to write a graph with the tools of this server, and the guide to the graph '
      + 'format',
    argsSchema: {
      request: z.string().optional().describe(
        'What the graph should make, like "a fern with '
          + 'wider fronds"'
      )
    }
  }, ({ request }) => ({
    messages: [{ role: 'user', content: { type: 'text', text: graphPrompt(request) } }]
  }));

  return server;
}

/**
 * Serves `createServer(dir)` on stdin and stdout. Stdout carries the protocol and nothing else:
 * the logs of the runtime and of the nodes go to stderr.
 */
export async function serveStdio(dir = nodesDir) {
  const log = (...args: unknown[]) => console.error(...args);
  console.log =
    console.info =
    console.debug =
    console.warn =
      log;
  await createServer(dir).connect(new StdioServerTransport());
}
