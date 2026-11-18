import {
  DEBUG_NODE,
  type Graph,
  type NodeDefinition,
  type NodeId,
  type NodeRegistry,
  type NodeValue,
  type SerializedNode
} from '@nodarium/types';
import {
  concatEncodedArrays,
  decodeFloat,
  isPath,
  PATH_HEADER_SIZE,
  pathPointCount,
  readNodeValue,
  readPathLink,
  setProbeListener
} from '@nodarium/utils';
import { itemsOf, POINTS_TAG } from '../render/triangles';
import { RESULT_TAGS } from '../result/decode-result';
import { MemoryRuntimeExecutor } from '../runtime-executor';

// `nodarium probe` and the instance lines of `nodarium run`: what comes out of a node, read
// through nodes of the command that sit on its edges and pass everything on.

/** Wraps an expression in `[9, tag, value]`, the nodes that read it hand every value to the host */
const PROBE_NODE = 'nodarium/cli/probe';
/** Keeps a copy of what goes through it */
const CAPTURE_NODE = 'nodarium/cli/capture';

/** The word of a probe expression, `PROBE` in `nodarium_utils` */
const PROBE = 9;

const VALUE_TYPES = new Set(['float', 'integer']);

const passOn = (type: string): NodeDefinition => ({
  id: type as NodeId,
  inputs: { input: { type: '*' }, tag: { type: 'integer', value: 0 } },
  outputs: ['*'],
  execute: () => {
    throw new Error(`${type} runs only inside a probe`);
  }
});

/** The registry of a graph, with the two nodes of the command, which `run` gives their code */
class ProbeRegistry implements NodeRegistry {
  status: NodeRegistry['status'] = 'ready';
  readonly own = new Map<string, NodeDefinition>();

  constructor(
    private inner: NodeRegistry,
    nodes: NodeDefinition[]
  ) {
    for (const node of nodes) this.own.set(node.id, node);
  }

  load = (ids: NodeId[]) => this.inner.load(ids.filter((id) => !this.own.has(id)));
  getNode = (id: string) => this.own.get(id) ?? this.inner.getNode(id);
  getAllNodes = () => [...this.inner.getAllNodes(), ...this.own.values()];
  register = (id: string, wasm: ArrayBuffer) => this.inner.register(id, wasm);
}

export type Spread = { min: number; max: number; mean: number };

function spread(values: number[]): Spread | undefined {
  if (!values.length) return undefined;
  let min = Infinity, max = -Infinity, sum = 0;
  for (const v of values) {
    min = Math.min(min, v);
    max = Math.max(max, v);
    sum += v;
  }
  return { min, max, mean: sum / values.length };
}

/** The instances of one block of a path list: how many, and how long and wide they are in m */
export type InstanceBlock = {
  count: number;
  /** of one instance along the axis it grows on, -z of its geometry */
  length?: Spread;
  width?: Spread;
};

function floatsOf(item: Int32Array, start: number, count: number) {
  return new Float32Array(item.buffer, item.byteOffset + start * 4, count);
}

function instanceBlock(item: Int32Array): InstanceBlock {
  const [, vertexCount, faceCount, count] = item;
  const start = 5 + faceCount * 3;
  const positions = floatsOf(item, start, vertexCount * 3);
  const matrices = floatsOf(item, start + vertexCount * 6, count * 16);
  let x = [Infinity, -Infinity], z = [Infinity, -Infinity];
  for (let v = 0; v < vertexCount; v++) {
    x = [Math.min(x[0], positions[v * 3]), Math.max(x[1], positions[v * 3])];
    z = [Math.min(z[0], positions[v * 3 + 2]), Math.max(z[1], positions[v * 3 + 2])];
  }
  const lengths: number[] = [], widths: number[] = [];
  for (let i = 0; i < count && vertexCount > 0; i++) {
    const m = matrices.subarray(i * 16, i * 16 + 16);
    lengths.push((z[1] - z[0]) * Math.hypot(m[8], m[9], m[10]));
    widths.push((x[1] - x[0]) * Math.hypot(m[0], m[1], m[2]));
  }
  return { count, length: spread(lengths), width: spread(widths) };
}

export type DataSummary = {
  /** per depth of the paths, 1 for a stem */
  paths?: { depth: number; count: number; length: Spread; points: Spread; grown: Spread }[];
  instances?: InstanceBlock[];
  /** extents along x, y and z of every geometry, in the units of the geometry */
  geometries?: { count: number; vertices: number; faces: number; size: Spread[] };
  points?: number;
};

function pathLength(path: Int32Array) {
  const count = pathPointCount(path);
  const f = floatsOf(path, PATH_HEADER_SIZE, count * 4);
  let length = 0;
  for (let i = 1; i < count; i++) {
    length += Math.hypot(
      f[i * 4] - f[i * 4 - 4],
      f[i * 4 + 1] - f[i * 4 - 3],
      f[i * 4 + 2] - f[i * 4 - 2]
    );
  }
  return length;
}

/** What a list of paths, geometry, instances or points holds */
export function summarizeData(data: Int32Array): DataSummary {
  const summary: DataSummary = {};
  const byDepth = new Map<number, { lengths: number[]; points: number[]; grown: number[] }>();
  const sizes: number[][] = [[], [], []];
  for (const item of itemsOf(data)) {
    if (isPath(item)) {
      const depth = item[1];
      const entry = byDepth.get(depth) ?? { lengths: [], points: [], grown: [] };
      entry.lengths.push(pathLength(item));
      entry.points.push(pathPointCount(item));
      entry.grown.push(readPathLink(item).grown);
      byDepth.set(depth, entry);
    } else if (item[0] === RESULT_TAGS.instances) {
      (summary.instances ??= []).push(instanceBlock(item));
    } else if (item[0] === RESULT_TAGS.geometry) {
      const [, vertices, faces] = item;
      const positions = floatsOf(item, 5 + faces * 3, vertices * 3);
      const geometries = summary.geometries ??= { count: 0, vertices: 0, faces: 0, size: [] };
      geometries.count++;
      geometries.vertices += vertices;
      geometries.faces += faces;
      for (let d = 0; d < 3; d++) {
        let lo = Infinity, hi = -Infinity;
        for (let v = 0; v < vertices; v++) {
          lo = Math.min(lo, positions[v * 3 + d]);
          hi = Math.max(hi, positions[v * 3 + d]);
        }
        if (vertices) sizes[d].push(hi - lo);
      }
    } else if (item[0] === POINTS_TAG) {
      summary.points = (summary.points ?? 0) + 1;
    }
  }
  if (summary.geometries) summary.geometries.size = sizes.map((s) => spread(s)!).filter(Boolean);
  if (byDepth.size) {
    summary.paths = [...byDepth.entries()].sort(([a], [b]) => a - b).map(([depth, e]) => ({
      depth,
      count: e.lengths.length,
      length: spread(e.lengths)!,
      points: spread(e.points)!,
      grown: spread(e.grown)!
    }));
  }
  return summary;
}

/** The values the nodes read from an output of value type, by the edge they came through */
export type ProbedReads = {
  /** the node and input the edge goes into, like `instance#45 size` */
  into: string;
  /** the node that read the values, when it isn't the one the edge goes into */
  readIn?: string;
  count: number;
  /** on how many paths they were read, the path a node worked on at the time */
  paths: number;
} & Spread;

export type ProbedOutput = {
  output: number;
  type: string;
  /** a value set on the node, the same wherever it is read */
  value?: number;
  /** the values an expression gave where it was read */
  reads?: ProbedReads[];
  /** what a path, geometry or points output holds */
  data?: DataSummary;
};

export type ProbeReport = { node: number; type: string; outputs: ProbedOutput[] };

type Edge = Graph['edges'][number];

const nodeName = (node: { id: number; type: string } | undefined, id: number) =>
  node ? `${node.type.split('/').pop()}#${id}` : `node ${id}`;

/**
 * The graph with a node of the command on every edge out of the outputs `outputs` of the node
 * `id`: a probe on every edge of a value, so every edge gets a tag of its own, and a capture on
 * those of a path, geometry or points output. An output nothing reads gets a capture of its own
 * that a debug node keeps running.
 */
function tappedGraph(graph: Graph, taps: { id: number; output: number; value: boolean }[]) {
  let next = Math.max(0, ...graph.nodes.map((n) => n.id)) + 1;
  const nodes: SerializedNode[] = [...graph.nodes];
  let edges: Edge[] = [...graph.edges];
  /** what each tag is: the edge it sits on, or the output it captures */
  const tags: { id: number; output: number; edge?: Edge }[] = [];
  for (const tap of taps) {
    const out = edges.filter((e) => e[0] === tap.id && e[1] === tap.output);
    if (tap.value && out.length) {
      for (const edge of out) {
        const probe = next++;
        nodes.push({ id: probe, type: PROBE_NODE, props: { tag: tags.length } });
        tags.push({ id: tap.id, output: tap.output, edge });
        edges = edges.map((e) => (e === edge ? [probe, 0, e[2], e[3]] : e));
        edges.push([tap.id, tap.output, probe, 'input']);
      }
      continue;
    }
    const capture = next++;
    nodes.push({ id: capture, type: CAPTURE_NODE, props: { tag: tags.length } });
    tags.push({ id: tap.id, output: tap.output });
    edges = edges.map((e) => (out.includes(e) ? [capture, 0, e[2], e[3]] : e));
    edges.push([tap.id, tap.output, capture, 'input']);
    if (!out.length) {
      const debug = next++;
      nodes.push({ id: debug, type: DEBUG_NODE, props: {} });
      edges.push([capture, 0, debug, 'input']);
    }
  }
  return { graph: { ...graph, nodes, edges } as Graph, tags };
}

/**
 * Runs `graph` with the outputs `taps` tapped. Gives what went through every capture by its tag,
 * and every value read from a probe with its tag, the path it was read on and the node that read
 * it. Throws when the graph fails.
 */
async function runTapped(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>,
  taps: { id: number; output: number; value: boolean }[]
) {
  const tapped = tappedGraph(graph, taps);
  const captured = new Map<number, Int32Array>();
  const read: { tag: number; value: number; path: number; node: number }[] = [];
  const tagOf = (inputs: NodeValue[]) => readNodeValue(inputs[1])[0];
  const probe: NodeDefinition = {
    ...passOn(PROBE_NODE),
    execute: (inputs) => {
      const value = readNodeValue(inputs[0]);
      captured.set(tagOf(inputs), value);
      // a plain value isn't an expression, some nodes take it for an unconnected input
      if (value.length === 1) return value;
      return concatEncodedArrays([PROBE, tagOf(inputs), value]);
    }
  };
  const capture: NodeDefinition = {
    ...passOn(CAPTURE_NODE),
    execute: (inputs) => {
      captured.set(tagOf(inputs), readNodeValue(inputs[0]));
      return inputs[0];
    }
  };
  const runtime = new MemoryRuntimeExecutor(new ProbeRegistry(registry, [probe, capture]));
  let node = -1;
  runtime.beforeNode = (id) => (node = id);
  setProbeListener((tag, value, path) => read.push({ tag, value, path, node }));
  try {
    await runtime.run(tapped.graph, settings);
  } finally {
    setProbeListener(undefined);
    runtime.releaseInstances();
  }
  return { tags: tapped.tags, captured, read };
}

function plainValue(data: Int32Array | undefined, type: string) {
  if (!data) return undefined;
  const words = data.length === 1
    ? data
    : data.length === 5 && data[0] === 0
    ? data.subarray(2, 3)
    : undefined;
  if (!words) return undefined;
  return type === 'float' ? decodeFloat(words[0]) : words[0];
}

/**
 * What comes out of the node `id` of a graph: for a float or integer output every value a node
 * read from it, by the edge it came through and the node that read it, for a path, geometry or
 * points output the paths by depth, the instances and the geometry in it. Throws when the graph
 * has no such node or fails.
 */
export async function probeGraph(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>,
  id: number
): Promise<ProbeReport> {
  const node = graph.nodes.find((n) => n.id === id);
  if (!node) {
    const ids = graph.nodes.map((n) => `${n.id} ${n.type.split('/').pop()}`).join(', ');
    throw new Error(`the graph has no node ${id}, it has the nodes ${ids}`);
  }
  await registry.load([node.type as NodeId]);
  const definition = registry.getNode(node.type);
  const types = (definition?.outputs ?? []).map((o) => (typeof o === 'string' ? o : o.type));
  const taps = types.map((type, output) => ({ id, output, value: VALUE_TYPES.has(type) }));
  const { tags, captured, read } = await runTapped(registry, graph, settings, taps);
  const names = new Map(graph.nodes.map((n) => [n.id, n]));

  const outputs = types.map((type, output): ProbedOutput => {
    if (!VALUE_TYPES.has(type)) {
      const tag = tags.findIndex((t) => t.output === output && !t.edge);
      const data = captured.get(tag);
      return { output, type, ...(data && { data: summarizeData(data) }) };
    }
    const groups = new Map<
      string,
      { into: string; readIn?: string; values: number[]; paths: Set<number> }
    >();
    for (const r of read) {
      const edge = tags[r.tag]?.edge;
      if (!edge || tags[r.tag].output !== output) continue;
      const into = `${nodeName(names.get(edge[2]), edge[2])} ${edge[3]}`;
      const readIn = r.node === edge[2] ? undefined : nodeName(names.get(r.node), r.node);
      const key = `${into}|${readIn}`;
      const group = groups.get(key) ?? { into, readIn, values: [], paths: new Set() };
      group.values.push(r.value);
      group.paths.add(r.path);
      groups.set(key, group);
    }
    if (groups.size) {
      return {
        output,
        type,
        reads: [...groups.values()].map((g) => ({
          into: g.into,
          ...(g.readIn && { readIn: g.readIn }),
          count: g.values.length,
          paths: g.paths.size,
          ...spread(g.values)!
        }))
      };
    }
    // nothing read an expression from it: a plain value, or an output nothing reads
    const tag = tags.findIndex((t) => t.output === output);
    return { output, type, value: plainValue(captured.get(tag), type) };
  });
  return { node: id, type: node.type, outputs };
}

/** The instances an instance node placed, by its id, in m */
export type InstancerSummary = {
  id: number;
  type: string;
  count: number;
  length?: Spread;
  width?: Spread;
};

const INSTANCERS = new Set(['max/plantarium/instance', 'max/plantarium/instance_from_points']);

function merge(blocks: InstanceBlock[]): Omit<InstancerSummary, 'id' | 'type'> {
  const count = blocks.reduce((sum, b) => sum + b.count, 0);
  const join = (key: 'length' | 'width') => {
    const parts = blocks.map((b) => b[key]).filter((s): s is Spread => !!s);
    if (!parts.length) return undefined;
    return {
      min: Math.min(...parts.map((p) => p.min)),
      max: Math.max(...parts.map((p) => p.max)),
      mean: parts.reduce((sum, p, i) => sum + p.mean * blocks[i].count, 0) / Math.max(1, count)
    };
  };
  return { count, length: join('length'), width: join('width') };
}

/**
 * Runs a graph and gives for every instance and instance_from_points node of it how many
 * instances it placed and how long and wide they are in m. Its output brings the instances of
 * the nodes before it along, only the blocks after those are its own.
 */
export async function instancers(
  registry: NodeRegistry,
  graph: Graph,
  settings: Record<string, unknown>
): Promise<InstancerSummary[]> {
  const nodes = graph.nodes.filter((n) => INSTANCERS.has(n.type));
  if (!nodes.length) return [];
  const taps = nodes.flatMap((n) => {
    const into = graph.edges.find((e) => e[2] === n.id && e[3] === 'plant');
    return [
      { id: n.id, output: 0, value: false },
      ...(into ? [{ id: into[0], output: into[1], value: false }] : [])
    ];
  });
  const { tags, captured } = await runTapped(registry, graph, settings, taps);
  const blocksOf = (id: number, output: number) => {
    const tag = tags.findIndex((t) => t.id === id && t.output === output && !t.edge);
    const data = captured.get(tag);
    return data ? summarizeData(data).instances ?? [] : undefined;
  };
  return nodes.flatMap((n) => {
    const out = blocksOf(n.id, 0);
    if (!out) return [];
    const into = graph.edges.find((e) => e[2] === n.id && e[3] === 'plant');
    const before = into ? blocksOf(into[0], into[1])?.length ?? 0 : 0;
    return [{ id: n.id, type: n.type, ...merge(out.slice(before)) }];
  });
}

const round = (v: number) => Number(v.toPrecision(3));

function range(s: Spread, unit = '') {
  return round(s.min) === round(s.max)
    ? `${round(s.min)}${unit}`
    : `${round(s.min)} to ${round(s.max)}${unit} (mean ${round(s.mean)})`;
}

/** The lines `nodarium run` prints for an instance node */
export function instancerLine(i: InstancerSummary) {
  const size = i.length && i.width
    ? `, ${range(i.length, ' m')} long, ${range(i.width, ' m')} wide`
    : '';
  return `${nodeName(i, i.id)}: ${i.count} instances${size}`;
}

function dataLines(data: DataSummary) {
  const lines: string[] = [];
  for (const p of data.paths ?? []) {
    const grown = p.grown.min === 1 && p.grown.max === 1 ? '' : `, grown ${range(p.grown)}`;
    lines.push(
      `depth ${p.depth}: ${p.count} paths, ${range(p.length, ' m')} long, ${
        range(p.points)
      } points${grown}`
    );
  }
  for (const [i, block] of (data.instances ?? []).entries()) {
    const size = block.length && block.width
      ? `, ${range(block.length, ' m')} long, ${range(block.width, ' m')} wide`
      : '';
    lines.push(`instances ${i + 1}: ${block.count}${size}`);
  }
  if (data.geometries) {
    const g = data.geometries;
    const [x, y, z] = g.size;
    lines.push(
      `${g.count} geometries, ${g.vertices} vertices, ${g.faces} faces`
        + (x ? `, x ${range(x)}, y ${range(y)}, z ${range(z)} across` : '')
    );
  }
  if (data.points !== undefined) lines.push(`${data.points} points`);
  return lines.length ? lines : ['nothing'];
}

/** What `nodarium probe` prints */
export function probeLines(name: string, report: ProbeReport) {
  const lines = [`${name}: ${nodeName({ id: report.node, type: report.type }, report.node)}`];
  for (const o of report.outputs) {
    const head = `  output ${o.output} (${o.type})`;
    if (o.reads) {
      lines.push(`${head}: read ${o.reads.reduce((sum, r) => sum + r.count, 0)} times`);
      for (const r of o.reads) {
        const where = r.readIn ? `, read in ${r.readIn}` : '';
        lines.push(
          `    into ${r.into}${where}: ${r.count} values on ${r.paths} paths, ${range(r)}`
        );
      }
    } else if (o.data) {
      lines.push(`${head}:`, ...dataLines(o.data).map((line) => `    ${line}`));
    } else if (o.value !== undefined) {
      lines.push(`${head}: ${round(o.value)}, a plain value`);
    } else {
      lines.push(`${head}: nothing read it`);
    }
  }
  return lines;
}
