import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, readdir, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CURRENT_GRAPH_VERSION } from '../graph-migrations';
import { stringifyGraph } from './graph-commands';
import { main } from './main';

const nodesDir = fileURLToPath(new URL('../../../../app/static/nodes', import.meta.url));

// --root takes no --nodes, the nodes of the package are those of the app here
vi.mock('../nodes-dir', async () => {
  const { fileURLToPath } = await import('node:url');
  return { nodesDir: fileURLToPath(new URL('../../../../app/static/nodes/', import.meta.url)) };
});

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

/** The command as it runs in a folder under `--root`, without --nodes */
async function inRoot(...args: string[]) {
  stdout = [];
  stderr = [];
  const code = await main(['--root', dir, ...args]);
  return { code, out: stdout.join('\n'), err: stderr.join('\n') };
}

/** The width and height of a PNG */
function pngSize(path: string) {
  const png = readFileSync(path);
  return [png.readUInt32BE(16), png.readUInt32BE(20)];
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

  it('sets a list with --set, in place of an edge into the input', async () => {
    const graph = await file('curved.json', {
      ...handWritten,
      nodes: [
        ...handWritten.nodes,
        { id: 4, type: 'max/plantarium/curve', props: { max: 0.5 } },
        { id: 5, type: 'max/plantarium/random', props: { min: 0.1, max: 0.2 } }
      ],
      edges: [...handWritten.edges, [4, 0, 1, 'length'], [5, 0, 4, 'value']]
    });
    const printed = await run('run', graph, '--print-graph', '--set', '4.curve=0,1,1,1');
    expect(printed.code).toBe(0);
    expect(JSON.parse(printed.out).nodes.find((n: { id: number }) => n.id === 4).props.curve)
      .toEqual([0, 1, 1, 1]);
    const set = await run('run', graph, '--json', '--set', '1.length=0.2');
    expect(set.err).toContain('curved: --set 1.length replaces the edge from curve#4');
    expect(JSON.parse(set.out).height).toBeCloseTo(0.2, 1);
  });

  it('takes the names of the options of a select and true or false for a boolean', async () => {
    const graph = await file('named.json', {
      ...handWritten,
      nodes: [...handWritten.nodes, {
        id: 4,
        type: 'max/plantarium/math',
        props: { op_type: 'multiply' }
      }]
    });
    expect((await run('run', graph, '--print-graph')).out).toContain('"op_type": 2');
    const flagged = await file('flagged.json', {
      ...handWritten,
      nodes: [...handWritten.nodes, { id: 4, type: 'max/plantarium/random', props: { once: true } }]
    });
    expect((await run('run', flagged, '--print-graph')).out).toContain('"once": 1');
    const wrong = await run('check', graph, '--set', '4.op_type=times');
    expect(wrong.code).toBe(1);
    expect(wrong.out).toContain(
      'math#4 "op_type" is "times", it takes the number or the name of an option: 0 "add", 1 '
    );
  });

  it('probes the values of a node and the paths of another', async () => {
    const graph = await file('probed.json', {
      ...handWritten,
      nodes: [
        ...handWritten.nodes,
        { id: 4, type: 'max/plantarium/random', props: { min: 0.01, max: 0.02 } }
      ],
      edges: [...handWritten.edges, [4, 0, 2, 'strength']]
    });
    const values = await run('probe', graph, '4');
    expect(values.code).toBe(0);
    expect(values.out).toMatch(
      /^probed: random#4\n {2}output 0 \(float\): read \d+ times\n {4}into noise#2 strength: /
    );
    const paths = await run('probe', graph, '1', '--json');
    expect(JSON.parse(paths.out).outputs[0].data.paths[0]).toMatchObject({ depth: 1, count: 1 });
    expect((await run('probe', graph, '9')).code).toBe(1);
    expect((await run('probe', graph)).code).toBe(1);
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

  it('writes a graph with every string as it was, lists of numbers and strings on one line', () => {
    const graph = {
      nodes: [{
        id: 1,
        position: [0, -2.5],
        props: { label: 'a,  b', names: ['a,  b', '[ 1, 2 ]'], empty: [], nested: [[1, 2], [3]] }
      }],
      edges: [[1, 0, 2, 'plant']],
      settings: {}
    };
    const text = stringifyGraph(graph);

    expect(JSON.parse(text)).toEqual(graph);
    expect(text).toContain('"position": [0, -2.5]');
    expect(text).toContain('[1, 0, 2, "plant"]');
    expect(text).toContain('"names": ["a,  b", "[ 1, 2 ]"]');
  });

  it('prints a link of a graph and reads the graph of a link', async () => {
    const graph = await file('hand.json', handWritten);
    const { code, out } = await run('link', graph, '--set', 'resolution.curve=8');
    expect(code).toBe(0);
    expect(out).toMatch(/^https:\/\/nodes\.max-richter\.dev\/#graph=[\w-]+$/);

    const linked = JSON.parse((await run('run', out, '--print-graph')).out);
    expect(linked.meta.title).toBe('hand');
    expect(linked.settings).toEqual({ 'resolution.curve': 8 });
    expect(linked.nodes.map((n: { props: unknown }) => n.props)).toEqual(
      handWritten.nodes.map((n) => n.props)
    );
    expect((await run('check', out)).out).toBe('hand: no issues');

    const preview = JSON.parse(
      (await run('link', 'fern', '--base', 'https://main.nodes.max-richter.dev/', '--json')).out
    );
    expect(preview.link).toMatch(/^https:\/\/main\.nodes\.max-richter\.dev\/#graph=/);
    expect(preview.length).toBe(preview.link.length);

    expect((await run('link', 'fern', '--base', 'nodes.dev')).code).toBe(1);
    expect((await run('check', `${out.slice(0, 80)}`)).code).toBe(1);
    expect(stderr.join()).toContain('the graph in the link is broken');
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

  it('lists the docs and prints one, with its links into the repository', async () => {
    const list = await run('docs');
    expect(list.code).toBe(0);
    expect(list.out.split('\n').map((line) => line.split(' ')[0])).toEqual([
      'graphs',
      'plant-templates',
      'plantarium',
      'myzel',
      'templates'
    ]);

    const myzel = await run('docs', 'myzel');
    expect(myzel.out).toMatch(/^# Myzel/);
    expect(myzel.out).toContain(
      '[Plantarium](https://git.max-richter.dev/max/nodarium/src/branch/main/docs/PLANTARIUM.md) '
        + '(`nodarium docs plantarium`)'
    );
    expect(myzel.out).not.toContain('](./');
    expect((await run('docs', 'PLANT_TEMPLATES.md')).out).toMatch(/^# Plant Templates/);
    expect((await run('docs', 'templates')).out).toContain(
      'fern: Fern (Plants, cm)\n  Nephrolepis exaltata (Boston fern): 22 fronds'
    );
    expect(JSON.parse((await run('docs', 'graphs', '--json')).out).text).toMatch(/^# Writing/);

    expect((await run('docs', 'gardening')).code).toBe(1);
    expect(stderr.join()).toContain('there is no doc gardening');
  });

  it('writes the graph of --print-graph into a new file', async () => {
    const target = join(dir, 'new', 'fern.json');
    const { code, out } = await run('run', 'fern', '--print-graph', '--write', target);
    expect(code).toBe(0);
    expect(out).toBe(`fern: wrote ${target}`);
    const written = await readFile(target, 'utf8');
    expect(written).toBe((await run('run', 'fern', '--print-graph')).out + '\n');

    expect(await run('run', 'fern', '--write', target, '--set', 'resolution.curve=8'))
      .toMatchObject(
        { code: 1, err: `${target} exists, --overwrite replaces it` }
      );
    expect(await readFile(target, 'utf8')).toBe(written);
    const replaced = await run(
      'run',
      'fern',
      '--write',
      target,
      '--overwrite',
      '--json',
      '--set',
      'resolution.curve=8'
    );
    expect(JSON.parse(replaced.out)).toEqual({ name: 'fern', file: target });
    expect(JSON.parse(await readFile(target, 'utf8')).settings['resolution.curve']).toBe(8);

    expect((await run('run', 'fern', 'daisy', '--write', join(dir, 'two.json'))).code).toBe(1);

    const dangling = join(dir, 'dangling.json');
    await symlink(join(dir, 'missing', 'fern.json'), dangling);
    const failed = await run('run', 'fern', '--write', dangling, '--overwrite');
    expect(failed.code).toBe(1);
    expect(failed.err).toMatch(new RegExp(`^fern: error can't write ${dangling}: ENOENT[^\\n]*$`));
    expect(existsSync(join(dir, 'two.json'))).toBe(false);
  });

  it('names the PNG with --name and draws the views of --views', async () => {
    const graph = await file('hand.json', handWritten);
    const named = JSON.parse(
      (await run('render', graph, '--out', dir, '--size', '64', '--name', 'try-2', '--json')).out
    );
    expect(named.screenshot).toBe(join(dir, 'try-2.png'));
    expect(pngSize(named.screenshot)).toEqual([192, 64]);

    const front = JSON.parse(
      (await run('render', graph, '--out', dir, '--size', '64', '--views', 'front', '--json')).out
    );
    expect(front.screenshot).toBe(join(dir, 'hand.png'));
    expect(pngSize(front.screenshot)).toEqual([64, 64]);
    await run('render', graph, '--out', dir, '--size', '64', '--views', 'side,top', '--below');
    expect(pngSize(join(dir, 'hand.png'))).toEqual([192, 64]);

    expect((await run('render', graph, '--views', 'back', '--out', dir)).code).toBe(1);
    expect((await run('render', graph, '--name', 'a/b', '--out', dir)).code).toBe(1);
    expect((await run('render', graph, 'fern', '--name', 'both', '--out', dir)).code).toBe(1);
    expect(stderr.join()).toContain('--name names one PNG');
  });

  it('draws the views of the default as they were without --views', async () => {
    const graph = await file('hand.json', handWritten);
    await run('render', graph, '--out', dir, '--size', '64', '--below', '--name', 'before');
    await run('render', graph, '--out', dir, '--size', '64', '--views', 'front,side,top,below');
    expect(await readFile(join(dir, 'hand.png'))).toEqual(await readFile(join(dir, 'before.png')));
  });

  it('draws a sheet of seeds and times and says which cell is which', async () => {
    const graph = await file('hand.json', {
      ...handWritten,
      nodes: [
        ...handWritten.nodes.slice(0, 1),
        { id: 2, type: 'max/plantarium/noise', props: { strength: 0.3 } },
        handWritten.nodes[2]
      ]
    });
    const { code, out } = await run(
      'render',
      graph,
      '--out',
      dir,
      '--size',
      '64',
      '--views',
      'front',
      '--seeds',
      '1,2,3',
      '--times',
      '0.5,1',
      '--json'
    );
    expect(code).toBe(0);
    const sheet = JSON.parse(out);
    expect(sheet).toMatchObject({
      name: 'hand',
      screenshot: join(dir, 'hand-sheet.png'),
      columns: 3,
      rows: 2,
      cellWidth: 64,
      cellHeight: 64
    });
    expect(sheet.cells.map((c: { seed: number; time: number }) => [c.seed, c.time])).toEqual([
      [1, 0.5],
      [2, 0.5],
      [3, 0.5],
      [1, 1],
      [2, 1],
      [3, 1]
    ]);
    expect(sheet.cells[4]).toMatchObject({ column: 1, row: 1, x: 66, y: 66 });
    expect(sheet.cells[0].triangles).toBeGreaterThan(0);
    expect(pngSize(sheet.screenshot)).toEqual([sheet.width, sheet.height]);
    expect([sheet.width, sheet.height]).toEqual([3 * 64 + 2 * 2, 2 * 64 + 2]);

    const text = await run('render', graph, '--out', dir, '--seeds', '1,2', '--name', 'seeds');
    const [line, cells] = text.out.split('\n');
    expect(line).toBe(`hand: ${join(dir, 'seeds.png')}, a sheet of 2 × 1 cells`);
    expect(JSON.parse(cells).cells.map((c: { seed: number }) => c.seed)).toEqual([1, 2]);
    expect(pngSize(join(dir, 'seeds.png'))).toEqual([2 * 3 * 256 + 4, 256]);

    expect((await run('render', graph, '--seeds', '1,x', '--out', dir)).code).toBe(1);
    expect((await run('render', graph, '--seeds', '1.5', '--out', dir)).code).toBe(1);
  });

  describe('with --root', () => {
    let outside: string;

    beforeEach(async () => {
      outside = await mkdtemp(join(tmpdir(), 'nodarium-outside-'));
      await writeFile(join(outside, 'secret.json'), JSON.stringify(handWritten));
      await mkdir(join(dir, 'screenshots'));
    });

    afterEach(() => rm(outside, { recursive: true }));

    it('reads and writes inside the root, relative to it', async () => {
      await file('hand.json', handWritten);
      expect(await inRoot('check', 'hand.json')).toMatchObject({ code: 0, out: 'hand: no issues' });
      expect((await inRoot('check', join(dir, 'hand.json'))).code).toBe(0);
      expect((await inRoot('check', 'fern')).out).toBe('fern: no issues');

      expect((await inRoot('run', 'fern', '--write', 'graphs/fern.json')).code).toBe(0);
      expect(existsSync(join(dir, 'graphs', 'fern.json'))).toBe(true);
      const rendered = await inRoot(
        'render',
        'graphs/fern.json',
        '--out',
        'shots',
        '--name',
        'f',
        '--size',
        '64',
        '--json'
      );
      expect(JSON.parse(rendered.out).screenshot).toBe(join(dir, 'shots', 'f.png'));
      expect((await inRoot('docs', 'graphs')).code).toBe(0);
    });

    it('reads a graph from stdin', async () => {
      vi.spyOn(process, 'stdin', 'get').mockReturnValue(
        Readable.from([Buffer.from(JSON.stringify(handWritten))]) as unknown as typeof process.stdin
      );
      expect(await inRoot('check', '-')).toMatchObject({ code: 0, out: 'stdin: no issues' });
    });

    it('refuses a path that leads outside, one line and exit code 1', async () => {
      await symlink(outside, join(dir, 'out-link'));
      await symlink(join(outside, 'secret.json'), join(dir, 'secret.json'));
      await symlink(join(outside, 'new.json'), join(dir, 'dangling.json'));
      await symlink(join(outside, 'new.png'), join(dir, 'screenshots', 'fern.png'));

      for (
        const target of [
          '../secret.json',
          join(outside, 'secret.json'),
          'secret.json',
          'out-link/secret.json'
        ]
      ) {
        const { code, out } = await inRoot('check', target, '--json');
        expect(code, target).toBe(1);
        expect(JSON.parse(out).error, target).toMatch(/is outside the root/);
      }
      const refused = [
        ['render', 'fern', '--out', '..'],
        ['render', 'fern', '--out', outside],
        ['render', 'fern', '--out', 'out-link'],
        ['render', 'fern', '--size', '32'],
        ['run', 'fern', '--write', '../fern.json'],
        ['run', 'fern', '--write', join(outside, 'fern.json')],
        ['run', 'fern', '--write', 'out-link/fern.json'],
        ['run', 'fern', '--write', 'dangling.json', '--overwrite'],
        ['--root', outside, 'nodes'],
        ['--nodes', nodesDir, 'nodes'],
        ['mcp']
      ];
      for (const args of refused) {
        const { code, err } = await inRoot(...args);
        expect(code, args.join(' ')).toBe(1);
        expect(err.split('\n'), args.join(' ')).toHaveLength(1);
      }
      expect((await inRoot('render', 'fern', '--size', '32')).err).toBe(
        `${join(dir, 'screenshots', 'fern.png')} is a link to nothing`
      );
      expect((await inRoot('mcp')).err).toBe('mcp is not allowed with --root');
      expect(await main(['--root', join(dir, 'nope'), 'nodes'])).toBe(1);
      expect(await readdir(outside)).toEqual(['secret.json']);
    });
  });
});
