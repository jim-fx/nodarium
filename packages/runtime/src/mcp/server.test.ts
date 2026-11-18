import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { createServer, graphPrompt } from './server';

const nodesDir = fileURLToPath(new URL('../../../../app/static/nodes', import.meta.url));

const handWritten = {
  version: CURRENT_GRAPH_VERSION,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { length: 0.5, thickness: 0.02 } },
    { id: 2, type: 'max/plantarium/noise', props: { strength: 0.01 } },
    { id: 3, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
};

let client: Client;

beforeAll(async () => {
  const [clientSide, serverSide] = InMemoryTransport.createLinkedPair();
  await createServer(nodesDir).connect(serverSide);
  client = new Client({ name: 'test', version: '1.0.0' });
  await client.connect(clientSide);
});

afterAll(() => client.close());

async function call(name: string, args: Record<string, unknown> = {}) {
  return (await client.callTool({ name, arguments: args })) as CallToolResult;
}

function textOf(result: CallToolResult) {
  return result.content.flatMap((c) => (c.type === 'text' ? [c.text] : [])).join('\n');
}

describe('nodarium mcp', () => {
  it('lists the tools with their arguments', async () => {
    const { tools } = await client.listTools();
    expect(tools.map((t) => t.name)).toEqual([
      'list_nodes',
      'get_node',
      'list_templates',
      'get_template',
      'check_graph',
      'run_graph',
      'probe_node',
      'render_graph',
      'graph_link',
      'get_docs'
    ]);
    const render = tools.find((t) => t.name === 'render_graph')!;
    expect(Object.keys(render.inputSchema.properties!)).toEqual(
      ['graph', 'seed', 'settings', 'node', 'output', 'views', 'size', 'focus', 'seeds', 'times']
    );
    expect(render.inputSchema.required).toEqual(['graph']);
    expect(tools.every((t) => t.annotations?.readOnlyHint && t.description)).toBe(true);
  });

  it('lists the nodes and shows one', async () => {
    const all = textOf(await call('list_nodes'));
    expect(all).toContain('max/plantarium/noise (path) -> path\n  Bends paths with smooth noise');
    expect(textOf(await call('list_nodes', { type: 'geometry' }))).toContain(
      'max/myzel/hyphae_to_mesh'
    );
    const leaves = textOf(await call('list_nodes', { query: 'LEAF' }));
    expect(leaves).toContain('max/plantarium/leaf');
    expect(leaves).not.toContain('max/plantarium/stem');

    const stem = textOf(await call('get_node', { id: 'stem noise' }));
    expect(stem.split('\n')[0]).toBe('max/plantarium/stem (points) -> path');
    expect(stem).toContain('\n\nmax/plantarium/noise (path) -> path');
    expect(await call('get_node', { id: 'stme' })).toMatchObject({ isError: true });
  });

  it('lists the templates and gives the graph of one', async () => {
    expect(textOf(await call('list_templates'))).toContain(
      'fern: Fern (Plants, cm)\n  Nephrolepis exaltata (Boston fern): 22 fronds'
    );
    const fern = JSON.parse(textOf(await call('get_template', { id: 'fern' })));
    expect(fern.version).toBe(CURRENT_GRAPH_VERSION);
    expect(fern.nodes.every((n: { position?: number[] }) => n.position?.length === 2)).toBe(true);
    expect(textOf(await call('get_template', { id: 'fren' }))).toContain(
      'there is no template fren'
    );
  });

  it('checks a graph and names every issue', async () => {
    expect(textOf(await call('check_graph', { graph: 'fern' }))).toBe('fern: no issues');
    const broken = {
      ...handWritten,
      meta: { title: 'Broken' },
      nodes: [{ id: 1, type: 'max/plantarium/stem', props: { lenght: 2 } }, handWritten.nodes[2]],
      edges: [[1, 0, 3, 'input']]
    };
    const checked = await call('check_graph', { graph: broken });
    expect(checked.isError).toBeFalsy();
    expect(textOf(checked)).toContain('broken: error unknown-prop: stem#1 has no input "lenght"');
  });

  it('runs a graph and answers with its size and the time of every node', async () => {
    const result = await call('run_graph', {
      graph: { ...handWritten, version: undefined },
      settings: { 'resolution.curve': 8 }
    });
    const lines = textOf(result).split('\n');
    const summary = JSON.parse(lines[0]);
    expect(summary).toMatchObject({ name: 'graph' });
    expect(summary.faces).toBeGreaterThan(0);
    expect(summary.height).toBeCloseTo(0.5, 1);
    expect(summary.nodes.map((n: { type: string }) => n.type)).toContain('max/plantarium/noise');
    expect(lines[1]).toContain('graph: warning no-version');

    const asText = await call('run_graph', { graph: JSON.stringify(handWritten), seed: 3 });
    expect(JSON.parse(textOf(asText)).vertices).toBeGreaterThan(summary.vertices);
  });

  it('probes the values a node gives and the paths and leaves it makes', async () => {
    const leaves = {
      version: CURRENT_GRAPH_VERSION,
      nodes: [
        { id: 1, type: 'max/plantarium/stem', props: { length: 0.5, thickness: 0.02 } },
        { id: 2, type: 'max/plantarium/random', props: { min: 0.5, max: 1 } },
        { id: 3, type: 'max/plantarium/leaf', props: { size: 0.1 } },
        { id: 4, type: 'max/plantarium/instance', props: { amount: 6 } },
        { id: 5, type: 'max/plantarium/output' }
      ],
      edges: [[1, 0, 4, 'plant'], [2, 0, 4, 'size'], [3, 0, 4, 'geometry'], [4, 0, 5, 'input']]
    };
    const values = textOf(await call('probe_node', { graph: leaves, node: 2 })).split('\n');
    expect(values[0]).toBe('graph: random#2');
    expect(values[2]).toMatch(/^ {4}into instance#4 size: 6 values on 1 paths, 0\.\d+ to 0\.\d+/);
    const paths = textOf(await call('probe_node', { graph: leaves, node: 4 }));
    expect(paths).toContain('depth 1: 1 paths, 0.5 m long');
    expect(paths).toMatch(/instances 1: 6, [\d.]+ to [\d.]+ m \(mean [\d.]+\) long/);
    const run = JSON.parse(textOf(await call('run_graph', { graph: leaves })).split('\n')[0]);
    expect(run.instancers).toMatchObject([{ id: 4, count: 6 }]);
  });

  it('answers a broken graph with an error that lists its issues', async () => {
    const cycle = { ...handWritten, edges: [...handWritten.edges, [2, 0, 1, 'points']] };
    for (const tool of ['run_graph', 'render_graph', 'graph_link']) {
      const result = await call(tool, { graph: cycle });
      expect(result.isError, tool).toBe(true);
      expect(textOf(result)).toMatch(/^graph has errors, nothing ran/);
      expect(textOf(result)).toContain('\ngraph: error cycle: ');
    }
    const notAGraph = await call('run_graph', { graph: { nodes: 'none' } });
    expect(notAGraph.isError).toBe(true);
    expect(await call('run_graph', { graph: 'not-a-template' })).toMatchObject({ isError: true });
  });

  it('refuses a graph larger than a link may carry', async () => {
    const huge = { ...handWritten, meta: { notes: 'x'.repeat(1024 * 1024) } };
    const result = await call('check_graph', { graph: huge });
    expect(result.isError).toBe(true);
    expect(textOf(result)).toContain('a graph may have 1024 KB');
  });

  it('renders a graph and the output of one of its nodes as an image', async () => {
    const result = await call('render_graph', { graph: handWritten, size: 64 });
    const [image, stats] = result.content;
    expect(image).toMatchObject({ type: 'image', mimeType: 'image/png' });
    const png = Buffer.from((image as { data: string }).data, 'base64');
    expect(png.subarray(1, 4).toString()).toBe('PNG');
    expect(png.readUInt32BE(16)).toBe(3 * 64);
    expect(JSON.parse((stats as { text: string }).text).triangles).toBeGreaterThan(0);

    const below = await call('render_graph', {
      graph: handWritten,
      size: 64,
      views: 'front-side-top-below'
    });
    const wide = Buffer.from((below.content[0] as { data: string }).data, 'base64');
    expect(wide.readUInt32BE(16)).toBe(4 * 64);

    const node = await call('render_graph', { graph: handWritten, node: 1, size: 64 });
    expect(JSON.parse((node.content[1] as { text: string }).text)).toMatchObject({
      node: 1,
      type: 'path'
    });
    expect(textOf(await call('render_graph', { graph: handWritten, node: 7 }))).toContain(
      'the graph has no node 7'
    );
    expect(await call('render_graph', { graph: handWritten, size: 4096 })).toMatchObject({
      isError: true
    });
  });

  it('draws the views of a list, and a sheet of seeds and times', async () => {
    const front = await call('render_graph', { graph: handWritten, size: 64, views: ['front'] });
    const png = Buffer.from((front.content[0] as { data: string }).data, 'base64');
    expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([64, 64]);

    const result = await call('render_graph', {
      graph: handWritten,
      size: 64,
      views: ['side', 'top'],
      seeds: [1, 2],
      times: [0.5, 1]
    });
    const sheet = JSON.parse((result.content[1] as { text: string }).text);
    expect(sheet).toMatchObject({ name: 'graph', columns: 2, rows: 2, cellWidth: 128 });
    expect(sheet.cells.map((c: { seed: number; time: number }) => [c.seed, c.time])).toEqual([
      [1, 0.5],
      [2, 0.5],
      [1, 1],
      [2, 1]
    ]);
    const image = Buffer.from((result.content[0] as { data: string }).data, 'base64');
    expect(image.readUInt32BE(16)).toBe(sheet.width);

    expect(await call('render_graph', { graph: handWritten, views: ['back'] })).toMatchObject({
      isError: true
    });
    const many = await call('render_graph', {
      graph: handWritten,
      seeds: [1, 2, 3, 4, 5],
      times: [0.2, 0.4, 0.6, 0.8]
    });
    expect(textOf(many)).toContain('a sheet has at most 16 cells, not 20');
    const large = await call('render_graph', {
      graph: handWritten,
      size: 1024,
      seeds: [1, 2, 3, 4, 5, 6]
    });
    expect(textOf(large)).toContain('million pixels');
  });

  it('gives the docs of the package', async () => {
    expect(textOf(await call('get_docs'))).toMatch(/^graphs +GRAPHS\.md/);
    const plants = textOf(await call('get_docs', { topic: 'plant-templates' }));
    expect(plants).toMatch(/^# Plant Templates/);
    expect(plants).toContain('## Fern: _Nephrolepis exaltata_');
    expect(await call('get_docs', { topic: 'gardening' })).toMatchObject({ isError: true });
  });

  it('gives a link that opens the graph in the app', async () => {
    const link = textOf(await call('graph_link', { graph: 'fern' }));
    expect(link).toMatch(/^https:\/\/nodes\.max-richter\.dev\/#graph=[\w-]+$/);
    expect(textOf(await call('check_graph', { graph: link }))).toBe('fern: no issues');
  });

  it('has the prompt to write a graph, with docs/GRAPHS.md', async () => {
    const { prompts } = await client.listPrompts();
    expect(prompts.map((p) => p.name)).toEqual(['write_graph']);
    const { messages } = await client.getPrompt({
      name: 'write_graph',
      arguments: { request: 'a fern with wider fronds' }
    });
    const prompt = (messages[0].content as { text: string }).text;
    expect(prompt).toContain('check_graph');
    expect(prompt).toContain('# Writing a graph by hand');
    expect(prompt.trimEnd()).toMatch(/What to make: a fern with wider fronds$/);
    expect(graphPrompt()).not.toContain('What to make');
  });
});
