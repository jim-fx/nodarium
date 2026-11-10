// Times the path nodes on thousands of strands, straight on the compiled node wasm
// (`pnpm build:nodes`), without the runtime and without meshing the paths into tubes.
//
// Usage: pnpm bench:paths [--counts 500,2000,5000,10000] [--runs 5] [--only noise,avoid,chain]
//                         [--dump dir] [--compare dir] [--wasm dir] [--radius m]
//                         [--templates champignon-groomed,chanterelle]
//   --counts     how many strands the nodes get
//   --runs       runs per measurement, the median is printed
//   --only       a part of the measurements: node names, `cap` for the cap nodes on a cap with
//                hundreds of rays, `chain` for the groomed stipe and cap, `templates`
//   --dump       writes the output of every measurement to dir
//   --compare    checks the output of every measurement against an earlier --dump
//   --radius     the free space avoid keeps between strands in the over-packed case of the chain,
//                0.2 mm by default, everywhere else avoid keeps none (the strands fit)
//   --wasm       where the node wasm is, as <dir>/<collection>/<node>.wasm, defaults to
//                static/nodes/max, the templates need it to end in static/nodes/max
//   --templates  the templates timed node by node through the runtime, hyphae_to_mesh apart
//
// The strands grow from a stem 5 cm tall and 7 mm thick, like a groomed stipe. The
// chain is the groomed mushroom at about 5000 strands: a stipe of strands that twist, break short and clump, and a cap
// whose guides are filled, waved, curled and opened, then filled with strands that break short.
// avoid runs on both, joined, once where the strands fit (radius 0) and once where they can't
// (radius --radius, about 2.4 times as much tube as the stipe holds).
import { createPerformanceStore, createWasmWrapper, readNodeValue } from '@nodarium/utils';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const root = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

const { values: options } = parseArgs({
  options: {
    counts: { type: 'string', default: '500,2000,5000,10000' },
    runs: { type: 'string', default: '5' },
    only: { type: 'string' },
    dump: { type: 'string' },
    compare: { type: 'string' },
    radius: { type: 'string', default: '0.0002' },
    wasm: { type: 'string', default: join(root, 'app', 'static', 'nodes', 'max') },
    templates: {
      type: 'string',
      default: 'champignon-groomed,chanterelle,oyster'
    }
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
type Run = (inputs: Inputs, props: Props, id: number) => Int32Array[];

const nodes = new Map<string, Run>();

function load(name: string): Run {
  if (nodes.has(name)) return nodes.get(name)!;
  const [collection, node] = name.split('/');
  const definition = JSON.parse(
    readFileSync(join(root, 'nodes', 'max', collection, node, 'src', 'definition.json'), 'utf8')
  );
  const bytes = readFileSync(join(options.wasm, collection, `${node}.wasm`));
  const wrapper = createWasmWrapper(
    bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength)
  );
  const run: Run = (links, props, id) => {
    const inputs = Object.entries(
      definition.inputs as Record<string, { type: string; value?: unknown }>
    ).map(([key, input]) => {
      if (links[key]) return links[key];
      if (input.type === 'creator') return Int32Array.of(id);
      if (key in props) return encode(input.type, props[key]);
      if (input.type === 'seed') return Int32Array.of(0);
      if (['path', 'force', 'points', 'pattern', 'mesh'].includes(input.type)) {
        return new Int32Array(0);
      }
      if (input.type === 'vec3') return encode('vec3', input.value ?? [0, 0, 0]);
      return encode(input.type, input.value ?? 0);
    });
    wrapper.reset();
    const outputs = wrapper.execute(inputs);
    return (Array.isArray(outputs) ? outputs : [outputs]).map(readNodeValue);
  };
  nodes.set(name, run);
  return run;
}

// every node gets a fixed id, so the ids and the random values of the paths don't depend on --only
const node = (id: number, name: string, inputs: Inputs = {}, props: Props = {}) =>
  load(name)(inputs, props, id)[0];
const nodeAll = (id: number, name: string, inputs: Inputs = {}, props: Props = {}) =>
  load(name)(inputs, props, id);

function median(values: number[]) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function time(id: number, name: string, inputs: Inputs, props: Props) {
  const times: number[] = [];
  let result: Int32Array[] = [];
  for (let i = 0; i < runs; i++) {
    const start = performance.now();
    result = load(name)(inputs, props, id);
    times.push(performance.now() - start);
  }
  return { ms: median(times), result: result[0], outputs: result };
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
  const file = label.replace(/[^A-Za-z0-9.-]+/g, '_') + '.bin';
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
/** One point at the tip of every path, where the cap sits */
const TIP = { amount: 1, lowest_point: 1, highest_point: 1 };
// as thick as in a groomed stipe, where 200 strands of 0.5 mm fill a 7 mm stipe, so
// more strands get thinner and fill the guide about as densely
const strands = (amount: number, guideThickness = STIPE.thickness) => ({
  amount,
  thickness: 0.0005 * (guideThickness / 0.007) * Math.sqrt(200 / amount),
  wobble: 0.25
});
// the cap strands of a groomed cap, 40 of 0.5 mm per guide, thinner when there are more
const capStrands = (amount: number) => ({
  amount,
  thickness: 0.0005 * Math.sqrt(40 / amount),
  wobble: 0.25
});
const STIPE_TWIST = { turns: 0.1 };
const BREAK = { chance: 0.5, lowest_break: 0.85, highest_break: 1 };
const AVOID = { strength: 1, smoothing: 0.5 };
const NEIGHBOUR_SENSING = { strength: 0.8, range: 0.003, bundling: 0.6, max_offset: 0.001 };
const MODIFIERS: [string, string, (force: Int32Array) => Inputs, Props][] = [
  ['plantarium/noise', '', () => ({}), { scale: 1, strength: 0.5 }],
  ['plantarium/gravity', '', () => ({}), { strength: 0.2 }],
  ['plantarium/apply_force', '', (force) => ({ force }), { strength: 1, stiffness: 0.3 }],
  ['plantarium/avoid', 'contacts', () => ({}), { ...AVOID, radius: 0, mode: 1 }],
  ['plantarium/avoid', 'density', () => ({}), { ...AVOID, radius: 0, mode: 2 }],
  ['plantarium/twist', '', () => ({}), STIPE_TWIST],
  ['plantarium/random_break', '', () => ({}), BREAK],
  ['plantarium/clump', '', () => ({}), { strength: 0.5, clumps: 12 }],
  ['plantarium/neighbour_sensing', '', () => ({}), NEIGHBOUR_SENSING],
  ['plantarium/branch', '', () => ({}), { amount: 2, length: 0.01, lowest_branch: 0.3 }],
  ['plantarium/fill_gaps', '', () => ({}), { spacing: 0.5 }],
  ['plantarium/separate', '', () => ({}), {}]
];
const pointForce = () =>
  node(3, 'plantarium/force', {}, {
    position: [0.02, 0.04, 0],
    strength: 1,
    radius: 0.05,
    falloff: 1
  });

const rows: string[][] = [];
const wanted = (...names: string[]) =>
  !only || only.some((o) => names.some((name) => name.endsWith(o)));
const label = (name: string, variant: string) => (variant ? `${name} (${variant})` : name);

for (const count of counts) {
  if (!wanted('nodes', 'strands', ...MODIFIERS.map((m) => m[0]), 'keep_apart')) break;
  const stem = node(1, 'plantarium/stem', {}, STIPE);
  const grown = time(2, 'myzel/strands', { plant: stem }, strands(count));
  if (wanted('nodes', 'strands')) {
    rows.push([
      'myzel/strands',
      `${count}`,
      grown.ms.toFixed(1),
      check(`strands-${count}`, grown.result)
    ]);
  }
  const force = pointForce();
  for (const [k, [name, variant, links, props]] of MODIFIERS.entries()) {
    if (!wanted('nodes', name)) continue;
    // separate takes every other strand back out, they all share one creator
    const extra = name.endsWith('separate') ? { like: grown.result } : links(force);
    const { ms, result } = time(10 + k, name, { plant: grown.result, ...extra }, props);
    const tag = `${name}${variant ? '-' + variant : ''}-${count}`;
    rows.push([label(name, variant), `${count}`, ms.toFixed(1), check(tag, result)]);
  }
  // keep_apart moves stems without a parent, a troop of `count` of them on one spot
  if (wanted('nodes', 'keep_apart') && count <= 1000) {
    const troop = node(4, 'plantarium/stem', {}, { ...STIPE, amount: count });
    const { ms, result } = time(30, 'plantarium/keep_apart', { plant: troop }, { radius: 0.01 });
    rows.push([
      'plantarium/keep_apart (stems)',
      `${count}`,
      ms.toFixed(1),
      check(`keep_apart-${count}`, result)
    ]);
  }
  console.error(`${count} strands done`);
}

// The cap nodes at their most rays: 128 from the cap and as many as fill_gaps adds
if (wanted('cap')) {
  const steps = new Map<string, number[]>();
  const outputs = new Map<string, Int32Array>();
  let rays = 0;
  for (let r = 0; r < runs; r++) {
    let nextId = 200;
    const step = (name: string, inputs: Inputs, props: Props) => {
      const t = performance.now();
      const out = nodeAll(nextId++, name, inputs, props);
      steps.set(name, [...(steps.get(name) ?? []), performance.now() - t]);
      outputs.set(name, out[0]);
      return out;
    };
    const stem = step('plantarium/stem', {}, STIPE)[0];
    const [tip] = step('plantarium/points_on_path', { plant: stem }, TIP);
    const [cap] = step('myzel/cap', { plant: stem, points: tip }, { rays: 128, radius: 0.03 });
    const [filled] = step('plantarium/fill_gaps', { plant: cap }, { spacing: 0.1 });
    rays = pathCount(filled);
    const [waved] = step('myzel/margin_waves', { cap: filled }, {});
    const [curled] = step('myzel/margin_curl', { cap: waved }, { amount: 0.2 });
    const [opened] = step('myzel/cap_open', { cap: curled }, { opening: 0.8 });
    const [margin] = step('myzel/cap_margin', { cap: opened }, {});
    step('myzel/membrane', { a: margin, b: stem }, { tear: 0.5 });
    const [pattern] = step('myzel/reaction_diffusion', {}, {
      radius: 0.03,
      inner_radius: 0.0065,
      spacing: 0.0005
    });
    step('myzel/lamellae', { cap: opened, pattern }, {});
  }
  for (const [name, t] of steps) {
    rows.push([
      `cap: ${name}`,
      `${rays}`,
      median(t).toFixed(1),
      check(`cap ${name}`, outputs.get(name)!)
    ]);
  }
}

if (wanted('chain')) {
  const times: number[] = [];
  const steps = new Map<string, number[]>();
  const outputs = new Map<string, Int32Array>();
  let guides = 0;
  for (let r = 0; r < runs; r++) {
    let nextId = 100;
    let total = 0;
    const step = (name: string, inputs: Inputs, props: Props, as = name, count = true) => {
      const t = performance.now();
      const out = nodeAll(nextId++, name, inputs, props);
      const ms = performance.now() - t;
      if (count) total += ms;
      steps.set(as, [...(steps.get(as) ?? []), ms]);
      outputs.set(as, out[0]);
      return out;
    };
    const [stem] = step('plantarium/stem', {}, STIPE);
    const [grown] = step('myzel/strands', { plant: stem }, strands(2500), 'stipe: myzel/strands');
    const [twisted] = step(
      'plantarium/twist',
      { plant: grown },
      STIPE_TWIST,
      'stipe: plantarium/twist'
    );
    const [stipe] = step(
      'plantarium/random_break',
      { plant: twisted },
      BREAK,
      'stipe: plantarium/random_break'
    );
    const [clumped] = step(
      'plantarium/clump',
      { plant: stipe },
      { strength: 0.5, clumps: 12 },
      'stipe: plantarium/clump'
    );
    const [tip] = step('plantarium/points_on_path', { plant: stem }, TIP, 'cap: points_on_path');
    const [cap] = step('myzel/cap', { plant: stem, points: tip }, { rays: 14 }, 'cap: myzel/cap');
    const [filled] = step(
      'plantarium/fill_gaps',
      { plant: cap },
      { spacing: 0.5 },
      'cap: plantarium/fill_gaps'
    );
    guides = pathCount(filled);
    const [waved] = step(
      'myzel/margin_waves',
      { cap: filled },
      {},
      'cap: myzel/margin_waves'
    );
    const [curled] = step(
      'myzel/margin_curl',
      { cap: waved },
      { amount: 0.2 },
      'cap: myzel/margin_curl'
    );
    const [opened] = step(
      'myzel/cap_open',
      { cap: curled },
      { opening: 1 },
      'cap: myzel/cap_open'
    );
    const [, rays] = step(
      'plantarium/separate',
      { plant: opened, like: stem },
      {},
      'cap: plantarium/separate'
    );
    const [grownCap] = step(
      'myzel/strands',
      { plant: rays },
      capStrands(Math.round(2500 / guides)),
      'cap: myzel/strands'
    );
    const [capStrandList] = step(
      'plantarium/random_break',
      { plant: grownCap },
      { ...BREAK, lowest_break: 0.95 },
      'cap: plantarium/random_break'
    );
    const [margin] = step('myzel/cap_margin', { cap: opened }, {}, 'veil: myzel/cap_margin');
    step('myzel/membrane', { a: margin, b: stem }, { tear: 0.5 }, 'veil: myzel/membrane');
    const [pattern] = step('myzel/reaction_diffusion', {}, {
      radius: 0.027,
      inner_radius: 0.00625,
      spacing: 0.001
    }, 'gills: myzel/reaction_diffusion');
    step('myzel/lamellae', { cap: opened, pattern }, {}, 'gills: myzel/lamellae');
    const [joined] = step(
      'plantarium/join',
      { a: clumped, b: capStrandList },
      {},
      'plantarium/join'
    );
    times.push(total);
    // the strands of the stipe and the cap are both two layers below the stem
    const avoid = { ...AVOID, depth: 2 };
    step(
      'plantarium/neighbour_sensing',
      { plant: joined },
      { ...NEIGHBOUR_SENSING, depth: 2 },
      'plantarium/neighbour_sensing',
      false
    );
    for (const [mode, name] of [[1, 'contacts'], [2, 'density']] as const) {
      step(
        'plantarium/avoid',
        { plant: joined },
        { ...avoid, radius: 0, mode },
        `avoid ${name}, radius 0`,
        false
      );
      step(
        'plantarium/avoid',
        { plant: joined },
        { ...avoid, radius: Number(options.radius), mode },
        `avoid ${name}, radius ${Number(options.radius) * 1000} mm (over-packed)`,
        false
      );
    }
  }
  for (const [name, t] of steps) {
    rows.push([
      `chain: ${name}`,
      '',
      median(t).toFixed(1),
      check(`chain ${name}`, outputs.get(name)!)
    ]);
  }
  const joined = outputs.get('plantarium/join')!;
  rows.push([
    `chain total without avoid (${pathCount(joined)} paths, ${guides} cap guides)`,
    '',
    median(times).toFixed(1),
    ''
  ]);
}

// The templates through the runtime, every node type summed, hyphae_to_mesh apart
if (wanted('templates')) {
  // the registry reads <cwd>/static/nodes/<collection>/<node>.wasm, the same nodes as --wasm
  process.chdir(dirname(dirname(dirname(options.wasm))));
  const { BenchmarkRegistry } = await import('../benchmark/benchmarkRegistry.ts');
  const { parseProjectFile } = await import('../src/lib/project-manager/project-file.ts');
  const { MemoryRuntimeExecutor } = await import('../src/lib/runtime/runtime-executor.ts');
  const { INTERNAL_PREFIX } = await import('@nodarium/types');
  for (const template of options.templates.split(',')) {
    const file = join(root, 'app', 'src', 'lib', 'graph-templates', `${template}.json`);
    const graph = parseProjectFile(JSON.parse(readFileSync(file, 'utf8'))).graph;
    const registry = new BenchmarkRegistry();
    // types inside groups are loaded by the runtime when it expands them
    await registry.load(
      graph.nodes.map((n) => n.type).filter((t) => !t.startsWith(INTERNAL_PREFIX))
    );
    const runtime = new MemoryRuntimeExecutor(registry);
    runtime.perf = createPerformanceStore();
    const perType = new Map<string, number[]>();
    const totals: number[] = [];
    const meshes: number[] = [];
    for (let r = 0; r < runs; r++) {
      runtime.perf.startRun();
      await runtime.execute(graph, { ...graph.settings, seed: 1 });
      runtime.perf.stopRun();
      const run = runtime.perf.get().at(-1)!;
      let total = 0;
      let mesh = 0;
      for (const [key, values] of Object.entries(run)) {
        if (!key.startsWith('node/')) continue;
        const sum = values.reduce((a, b) => a + b, 0);
        const type = key.slice('node/max/'.length);
        perType.set(type, [...(perType.get(type) ?? []), sum]);
        if (type === 'myzel/hyphae_to_mesh') mesh += sum;
        else total += sum;
      }
      totals.push(total);
      meshes.push(mesh);
    }
    const sorted = [...perType].sort((a, b) => median(b[1]) - median(a[1]));
    for (const [type, t] of sorted) {
      if (type === 'myzel/hyphae_to_mesh' || median(t) < 0.5) continue;
      rows.push([`${template}: ${type}`, '', median(t).toFixed(1), '']);
    }
    rows.push([`${template}: total without hyphae_to_mesh`, '', median(totals).toFixed(1), '']);
    rows.push([`${template}: hyphae_to_mesh (context)`, '', median(meshes).toFixed(1), '']);
    console.error(`${template} done`);
  }
}

const header = ['node', 'paths', 'median ms', options.compare ? 'output' : ''];
const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
for (const row of [header, ...rows]) {
  console.log(row.map((c, i) => c.padEnd(widths[i])).join('  ').trimEnd());
}
