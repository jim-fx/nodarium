import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { main } from './main';

const nodesDir = fileURLToPath(new URL('../../../../app/static/nodes', import.meta.url));

// a stem with noise, written by hand: no positions
const handWritten = {
  version: CURRENT_GRAPH_VERSION,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { length: 0.5, thickness: 0.02 } },
    { id: 2, type: 'max/plantarium/noise', props: { strength: 0.01 } },
    { id: 3, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
};

let dir: string;
let stdout: string[];
let stderr: string[];

async function run(...args: string[]) {
  stdout = [];
  stderr = [];
  const code = await main([...args, '--nodes', nodesDir]);
  return { code, out: stdout.join('\n'), err: stderr.join('\n') };
}

async function file(name: string, content: unknown) {
  const path = join(dir, name);
  await writeFile(path, JSON.stringify(content));
  return path;
}

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'nodarium-cli-'));
  vi.spyOn(console, 'log').mockImplementation((...args) => void stdout.push(args.join(' ')));
  vi.spyOn(console, 'error').mockImplementation((...args) => void stderr.push(args.join(' ')));
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(dir, { recursive: true });
});

describe('nodarium', () => {
  it('prints its usage and exits with 1 without a command', async () => {
    expect((await run()).code).toBe(1);
    expect((await run('grow', 'fern')).code).toBe(1);
    expect(stderr.join()).toContain('there is no command grow');
  });

  it('lists the nodes and shows one with its inputs', async () => {
    const list = await run('nodes');
    expect(list.code).toBe(0);
    expect(list.out).toContain(
      'max/plantarium/noise (path) -> path\n  Bends paths with smooth noise'
    );

    const stem = await run('nodes', 'stem');
    expect(stem.out.split('\n')[0]).toBe('max/plantarium/stem (points) -> path');
    expect(stem.out).toContain('  amount: integer = 1 [1..1000]\n    The amount of stems');

    const meshes = JSON.parse((await run('nodes', '--type', 'mesh', '--json')).out);
    expect(meshes.map((n: { id: string }) => n.id)).toContain('max/myzel/join_meshes');

    expect((await run('nodes', 'stme')).code).toBe(1);
  });

  it('checks a graph, exits with 1 on an error and names all issues', async () => {
    expect(await run('check', 'fern')).toMatchObject({ code: 0, out: 'fern: no issues' });

    const broken = await file('broken.json', {
      ...handWritten,
      nodes: [{ id: 1, type: 'max/plantarium/stem', props: { lenght: 2 } }, handWritten.nodes[2]],
      edges: [[1, 0, 3, 'input']]
    });
    const { code, out } = await run('check', broken);
    expect(code).toBe(1);
    expect(out).toContain('broken: error unknown-prop: stem#1 has no input "lenght"');

    const json = JSON.parse((await run('check', broken, '--json')).out);
    expect(json).toMatchObject({ name: 'broken', ok: false });
    expect(json.issues[0].suggestion).toBe('length');
  });

  it('runs a graph without positions and prints its size', async () => {
    const graph = await file('hand.json', handWritten);
    const { code, out } = await run('run', graph, '--json', '--timings');
    expect(code).toBe(0);
    const summary = JSON.parse(out);
    expect(summary.faces).toBeGreaterThan(0);
    expect(summary.height).toBeCloseTo(0.5, 1);
    expect(summary.nodes.map((n: { type: string }) => n.type)).toContain('max/plantarium/noise');
    expect(summary.nodes.every((n: { bytes: number }) => n.bytes > 0)).toBe(true);
  });

  it('changes settings and inputs with --set', async () => {
    const graph = await file('hand.json', handWritten);
    const short = JSON.parse((await run('run', graph, '--json', '--set', '1.length=0.2')).out);
    expect(short.height).toBeCloseTo(0.2, 1);
    expect((await run('run', graph, '--set', '9.length=1')).code).toBe(1);
  });

  it('prints the graph with a version and a position for every node', async () => {
    const graph = await file('hand.json', { ...handWritten, version: undefined });
    const { code, out, err } = await run(
      'run',
      graph,
      '--print-graph',
      '--set',
      'resolution.curve=8'
    );
    expect(code).toBe(0);
    expect(err).toContain('no-version');
    const printed = JSON.parse(out);
    expect(printed.version).toBe(CURRENT_GRAPH_VERSION);
    expect(printed.settings).toEqual({ 'resolution.curve': 8 });
    expect(printed.nodes.every((n: { position?: number[] }) => n.position?.length === 2)).toBe(
      true
    );
  });

  it('renders a graph and the output of one of its nodes', async () => {
    const graph = await file('hand.json', handWritten);
    const whole = JSON.parse(
      (await run('render', graph, '--out', dir, '--size', '64', '--json')).out
    );
    expect(whole.screenshot).toBe(join(dir, 'hand.png'));
    expect(existsSync(whole.screenshot)).toBe(true);

    const node = JSON.parse(
      (await run('render', graph, '--node', '1', '--out', dir, '--size', '64', '--json')).out
    );
    expect(node).toMatchObject({ node: 1, type: 'path', screenshot: join(dir, 'hand-node1.png') });
    expect(node.triangles).toBeGreaterThan(0);

    expect((await run('render', graph, '--node', '7', '--out', dir)).code).toBe(1);
  });

  it('does not run a graph with an error', async () => {
    const graph = await file('cycle.json', {
      ...handWritten,
      edges: [...handWritten.edges, [2, 0, 1, 'points']]
    });
    const { code, out, err } = await run('run', graph, '--json');
    expect(code).toBe(1);
    expect(JSON.parse(out).error).toContain('stem#1');
    expect(err).toContain('error');
  });

  it('places the nodes that have no position', async () => {
    const graph = await file('hand.json', handWritten);
    expect((await run('format', graph, '--check')).code).toBe(1);
    expect((await run('format', graph)).out).toBe('hand: placed 3 nodes');
    const formatted = JSON.parse(await readFile(graph, 'utf8'));
    expect(formatted.nodes.map((n: { position: number[] }) => n.position[0])).toEqual([
      -60,
      -30,
      0
    ]);
    expect((await run('format', graph, '--check')).code).toBe(0);
  });
});
