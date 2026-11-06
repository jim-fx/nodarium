// Times the path nodes on thousands of strands, straight on the compiled node wasm
// (`pnpm build:nodes`), without the runtime and without meshing the paths into tubes.
//
// Usage: pnpm bench:paths [--counts 500,2000,10000] [--runs 5] [--only noise,avoid]
//                         [--dump dir] [--compare dir] [--wasm dir] [--radius m]
//   --counts   how many strands the nodes get
//   --runs     runs per measurement, the median is printed
//   --only     a part of the nodes, `chain` is the stipe and cap through the modifiers
//   --dump     writes the output of every measurement to dir
//   --compare  checks the output of every measurement against an earlier --dump
//   --radius   the free space avoid keeps between strands, 0.2 mm by default
//   --wasm     where the node wasm is, as <dir>/<collection>/<node>.wasm, defaults to static/nodes
//
// The strands grow from a stem 5 cm tall and 7 mm thick, like the groomed stipe template. The
// chain is a stand-in for a groomed mushroom until there is a cap node: a stipe of 2500 strands
// and a cap of 24 umbel rays with 104 strands each, joined and bent by noise, gravity,
// apply_force and avoid.
import { createWasmWrapper, readNodeValue } from '@nodarium/utils';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

const { values: options } = parseArgs({
  options: {
    counts: { type: 'string', default: '500,2000,10000' },
    runs: { type: 'string', default: '5' },
    only: { type: 'string' },
    dump: { type: 'string' },
    compare: { type: 'string' },
    radius: { type: 'string', default: '0.0002' },
    wasm: { type: 'string', default: join(root, 'app', 'static', 'nodes', 'max') }
  }
});
const counts = options.counts.split(',').map(Number);
const runs = Number(options.runs);
const only = options.only?.split(',');

const f32 = new Float32Array(1);
const i32 = new Int32Array(f32.buffer);
const float = (v: number) => ((f32[0] = v), i32[0]);

function encode(type: string, value: unknown): Int32Array {
  switch (type) {
    case 'float':
      return Int32Array.of(float(value as number));
    case 'vec3':
    case 'curve':
    case 'shape': {
      const v = value as number[];
      return Int32Array.from([0, v.length + 1, ...v.map(float), 1, 1]);
    }
    case 'boolean':
      return Int32Array.of(value ? 1 : 0);
    default:
      return Int32Array.of(value as number);
  }
}

type Inputs = Record<string, Int32Array>;
type Props = Record<string, unknown>;

const nodes = new Map<string, { run: (inputs: Inputs, props: Props, id: number) => Int32Array }>();

function load(name: string) {
  if (nodes.has(name)) return nodes.get(name)!;
  const [collection, node] = name.split('/');
  const definition = JSON.parse(
    readFileSync(join(root, 'nodes', 'max', collection, node, 'src', 'definition.json'), 'utf8')
  );
  const bytes = readFileSync(join(options.wasm, collection, `${node}.wasm`));
  const wrapper = createWasmWrapper(
    bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)
  );
  const run = (links: Inputs, props: Props, id: number) => {
    const inputs = Object.entries(
      definition.inputs as Record<string, { type: string; value?: unknown }>
    ).map(([key, input]) => {
      if (links[key]) return links[key];
      if (input.type === 'creator') return Int32Array.of(id);
      if (key in props) return encode(input.type, props[key]);
      if (input.type === 'seed') return Int32Array.of(0);
      if (['path', 'force'].includes(input.type)) return new Int32Array(0);
      if (input.type === 'vec3') return encode('vec3', input.value ?? [0, 0, 0]);
      return encode(input.type, input.value ?? 0);
    });
    wrapper.reset();
    const outputs = wrapper.execute(inputs);
    // the first output, umbel also puts out the tips of its rays
    return readNodeValue(Array.isArray(outputs) ? outputs[0] : outputs);
  };
  nodes.set(name, { run });
  return nodes.get(name)!;
}

// every node gets a fixed id, so the ids and the random values of the paths don't depend on --only
const node = (id: number, name: string, inputs: Inputs = {}, props: Props = {}) =>
  load(name).run(inputs, props, id);

function median(values: number[]) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function time(id: number, name: string, inputs: Inputs, props: Props) {
  const n = load(name);
  const times: number[] = [];
  let result = new Int32Array(0);
  for (let i = 0; i < runs; i++) {
    const start = performance.now();
    result = n.run(inputs, props, id);
    times.push(performance.now() - start);
  }
  return { ms: median(times), result };
}

// Splits a list of paths into its paths, like split_args
function paths(list: Int32Array) {
  const out: Int32Array[] = [];
  let i = 2;
  while (i < list.length - 2) {
    const end = i + list[i + 1] + 3;
    out.push(list.subarray(i, end));
    i = end;
  }
  return out;
}

const pathCount = (list: Int32Array) => paths(list).length;

// The largest distance between two points of two lists of paths, Infinity if they don't match up
function difference(a: Int32Array, b: Int32Array) {
  if (a.length !== b.length) return Infinity;
  const fa = new Float32Array(a.buffer, a.byteOffset, a.length);
  const fb = new Float32Array(b.buffer, b.byteOffset, b.length);
  let worst = 0;
  const [pa, pb] = [paths(a), paths(b)];
  if (pa.length !== pb.length) return Infinity;
  for (let p = 0; p < pa.length; p++) {
    const offset = pa[p].byteOffset / 4 - a.byteOffset / 4;
    // header and brackets have to match exactly, the points may move a little
    for (let k = 0; k < 9; k++) if (pa[p][k] !== pb[p][k]) return Infinity;
    for (let k = 9; k < pa[p].length - 2; k++) {
      worst = Math.max(worst, Math.abs(fa[offset + k] - fb[offset + k]));
    }
  }
  return worst;
}

function check(label: string, result: Int32Array) {
  const file = label.replaceAll('/', '_').replaceAll(' ', '_') + '.bin';
  if (options.dump) {
    mkdirSync(options.dump, { recursive: true });
    writeFileSync(join(options.dump, file), result);
  }
  if (!options.compare) return '';
  const path = join(options.compare, file);
  if (!existsSync(path)) return 'no reference';
  const bytes = readFileSync(path);
  const reference = new Int32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4);
  if (reference.length === result.length && reference.every((v, i) => v === result[i])) {
    return 'identical';
  }
  return `differs by ${difference(reference, result).toExponential(2)} m`;
}

const STIPE = { length: 0.05, thickness: 0.007, taper: 0.25, resolution_curve: 24 };
// as thick as in the groomed stipe template, where 200 strands of 0.5 mm fill a 7 mm stipe, so
// more strands get thinner and fill the guide about as densely
const strands = (amount: number, guideThickness = STIPE.thickness) => ({
  amount,
  thickness: 0.0005 * (guideThickness / 0.007) * Math.sqrt(200 / amount),
  thickness_variation: 0.5,
  twist: 0.1,
  wobble: 0.25,
  length_variation: 0.3
});
const MODIFIERS: [string, (force: Int32Array) => Inputs, Props][] = [
  ['plantarium/noise', () => ({}), { scale: 1, strength: 0.5 }],
  ['plantarium/gravity', () => ({}), { strength: 0.2 }],
  ['plantarium/apply_force', (force) => ({ force }), { strength: 1, stiffness: 0.3 }],
  ['plantarium/avoid', () => ({}), { radius: Number(options.radius), strength: 1, smoothing: 0.5 }],
  ['plantarium/clump', () => ({}), { strength: 0.5, clumps: 12 }],
  ['plantarium/branch', () => ({}), { amount: 2, length: 0.01, lowest_branch: 0.3 }]
];
const pointForce = () =>
  node(3, 'plantarium/force', {}, {
    position: [0.02, 0.04, 0],
    strength: 1,
    radius: 0.05,
    falloff: 1
  });

const rows: string[][] = [];
const wanted = (name: string) => !only || only.some((o) => name.endsWith(o));

for (const count of counts) {
  const stem = node(1, 'plantarium/stem', {}, STIPE);
  const grown = time(2, 'myzel/strands', { plant: stem }, strands(count));
  if (wanted('strands')) {
    rows.push([
      'myzel/strands',
      `${count}`,
      grown.ms.toFixed(1),
      check(`strands-${count}`, grown.result)
    ]);
  }
  const force = pointForce();
  for (const [k, [name, links, props]] of MODIFIERS.entries()) {
    if (!wanted(name)) continue;
    const { ms, result } = time(10 + k, name, { plant: grown.result, ...links(force) }, props);
    rows.push([name, `${count}`, ms.toFixed(1), check(`${name}-${count}`, result)]);
  }
  console.error(`${count} strands done`);
}

if (wanted('chain')) {
  const times: number[] = [];
  const steps = new Map<string, number[]>();
  const outputs = new Map<string, Int32Array>();
  for (let r = 0; r < runs; r++) {
    const start = performance.now();
    let nextId = 100;
    const step = (name: string, inputs: Inputs, props: Props, label = name) => {
      const t = performance.now();
      const out = load(name).run(inputs, props, nextId++);
      steps.set(label, [...(steps.get(label) ?? []), performance.now() - t]);
      outputs.set(label, out);
      return out;
    };
    const stem = step('plantarium/stem', {}, STIPE);
    const stipe = step('myzel/strands', { plant: stem }, strands(2500), 'stipe strands');
    const rays = step('plantarium/umbel', { plant: stem }, {
      amount: 24,
      length: 0.03,
      length_variation: 0,
      opening: 170,
      thickness: 0.002,
      resolution_curve: 24
    });
    const gills = step('myzel/strands', { plant: rays }, strands(104, 0.002), 'cap strands');
    let plant = step('plantarium/join', { a: stipe, b: gills }, {});
    const force = step('plantarium/force', {}, {
      position: [0.02, 0.04, 0],
      strength: 1,
      radius: 0.05,
      falloff: 1
    });
    // the cap strands are one layer deeper than the stipe strands, depth 2 bends both but not
    // the stem they grow on
    for (const [name, links, props] of MODIFIERS.slice(0, 4)) {
      plant = step(name, { plant, ...links(force) }, { ...props, depth: 2 });
    }
    times.push(performance.now() - start);
  }
  for (const [name, t] of steps) {
    rows.push([
      `chain: ${name}`,
      '',
      median(t).toFixed(1),
      check(`chain ${name}`, outputs.get(name)!)
    ]);
  }
  const result = outputs.get('plantarium/avoid')!;
  rows.push([`chain total (${pathCount(result)} paths)`, '', median(times).toFixed(1), '']);
}

const header = ['node', 'paths', 'median ms', options.compare ? 'output' : ''];
const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
for (const row of [header, ...rows]) {
  console.log(row.map((c, i) => c.padEnd(widths[i])).join('  ').trimEnd());
}
