// Renders graph templates or project files to PNG screenshots, without a browser.
// It executes the graph with the compiled nodes in static/nodes (`pnpm build:nodes`) and
// rasterizes the result in software: front, side and a view from above, next to each other.
// Stems are brown, instanced geometry (leaves, flowers) is green.
//
// Usage: pnpm render-graph <target...> [--out dir] [--size px] [--seed n]
//   target   name of a template in src/lib/graph-templates (fern, palm, ...), `templates` for
//            all json templates, or the path of a graph or an exported `.nodarium.json` project
//   --out    directory for the screenshots, defaults to ./screenshots
//   --size   size of a single view in pixels, defaults to 512
//   --seed   seed of the random nodes, defaults to 1, the same seed gives the same plant
//
// Prints one line of JSON per target with the path of the screenshot and the size of the plant.
import type { Graph, NodeId } from '@nodarium/types';
import { splitNestedArray } from '@nodarium/utils';
import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { deflateSync } from 'node:zlib';

import { BenchmarkRegistry } from '../benchmark/benchmarkRegistry.ts';
import { parseProjectFile } from '../src/lib/project-manager/project-file.ts';
import { MemoryRuntimeExecutor } from '../src/lib/runtime/runtime-executor.ts';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const TEMPLATE_DIR = join(root, 'src', 'lib', 'graph-templates');

const { values: options, positionals: targets } = parseArgs({
  allowPositionals: true,
  options: {
    out: { type: 'string', default: join(process.cwd(), 'screenshots') },
    size: { type: 'string', default: '512' },
    seed: { type: 'string', default: '1' }
  }
});

if (targets.length === 0) {
  console.error(
    'Usage: pnpm render-graph <template|templates|file.json...> [--out dir] [--size px] [--seed n]'
  );
  process.exit(1);
}

type Vec3 = [number, number, number];
type Triangle = { a: Vec3; b: Vec3; c: Vec3; instanced: boolean };

const STEM_COLOR = [125, 95, 60];
const INSTANCE_COLOR = [70, 140, 60];
const BACKGROUND = [236, 233, 224];
const GROUND = [150, 130, 100];
// rendered larger and scaled down, so thin stems don't break up into dots
const SUPERSAMPLING = 2;

async function resolveTargets(names: string[]) {
  const files: string[] = [];
  for (const name of names) {
    if (name === 'templates') {
      const templates = (await readdir(TEMPLATE_DIR)).filter((f) => f.endsWith('.json'));
      files.push(...templates.map((f) => join(TEMPLATE_DIR, f)));
    } else if (name.endsWith('.json')) {
      files.push(resolve(name));
    } else {
      files.push(join(TEMPLATE_DIR, name + '.json'));
    }
  }
  return files;
}

async function loadGraph(file: string): Promise<Graph> {
  return parseProjectFile(JSON.parse(await readFile(file, 'utf8'))).graph;
}

async function execute(graph: Graph, seed: number) {
  const registry = new BenchmarkRegistry();
  await registry.load(graph.nodes.map((n) => n.type) as NodeId[]);
  const runtime = new MemoryRuntimeExecutor(registry);
  // the runtime picks a random seed when it is created, a fixed one makes screenshots comparable
  (runtime as unknown as { seed: number }).seed = seed;
  return runtime.execute(graph, { ...graph.settings, randomSeed: false });
}

function floats(data: Int32Array, offset: number, length: number) {
  return new Float32Array(data.buffer, data.byteOffset + offset * 4, length);
}

/** Collects the triangles of the geometry and the instances in the result of the output node */
function collectTriangles(result: Int32Array) {
  const triangles: Triangle[] = [];
  let instances = 0;

  for (const part of splitNestedArray(result)) {
    // plain geometry: [1, vertices, faces, ...faces, ...positions, ...normals]
    // instances: [2, vertices, faces, instances, depth, ...faces, ...positions, ...normals, ...matrices]
    const instanced = part[0] === 2;
    if (part[0] !== 1 && !instanced) continue;

    let index = 1;
    const vertexCount = part[index++];
    const faceCount = part[index++];
    const instanceCount = instanced ? part[index++] : 1;
    if (instanced) index++;
    const faces = part.subarray(index, index + faceCount * 3);
    index += faceCount * 3;
    const positions = floats(part, index, vertexCount * 3);
    index += vertexCount * 6;
    const matrices = instanced ? floats(part, index, instanceCount * 16) : undefined;
    instances += instanced ? instanceCount : 0;

    for (let i = 0; i < instanceCount; i++) {
      const m = matrices?.subarray(i * 16, i * 16 + 16);
      const vertex = (v: number): Vec3 => {
        const x = positions[v * 3], y = positions[v * 3 + 1], z = positions[v * 3 + 2];
        if (!m) return [x, y, z];
        return [
          m[0] * x + m[4] * y + m[8] * z + m[12],
          m[1] * x + m[5] * y + m[9] * z + m[13],
          m[2] * x + m[6] * y + m[10] * z + m[14]
        ];
      };
      for (let f = 0; f < faceCount; f++) {
        triangles.push({
          a: vertex(faces[f * 3]),
          b: vertex(faces[f * 3 + 1]),
          c: vertex(faces[f * 3 + 2]),
          instanced
        });
      }
    }
  }

  return { triangles, instances };
}

function bounds(triangles: Triangle[]) {
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const t of triangles) {
    for (const v of [t.a, t.b, t.c]) {
      for (let d = 0; d < 3; d++) {
        min[d] = Math.min(min[d], v[d]);
        max[d] = Math.max(max[d], v[d]);
      }
    }
  }
  return { min, max };
}

/** Front, side and from above, each view gets the same scale so the views can be compared */
const VIEWS = [{ yaw: 0, pitch: 0 }, { yaw: Math.PI / 2, pitch: 0 }, { yaw: 0.6, pitch: -1.15 }];

function rasterize(triangles: Triangle[], viewSize: number) {
  const size = viewSize * SUPERSAMPLING;
  const width = size * VIEWS.length;
  const pixels = new Uint8Array(width * size * 3);
  for (let i = 0; i < pixels.length; i += 3) pixels.set(BACKGROUND, i);
  const depth = new Float32Array(width * size).fill(Infinity);

  const { min, max } = bounds(triangles);
  // the ground stays in the picture, the plant grows from the origin
  const bottom = Math.min(min[1], 0), top = Math.max(max[1], 0);
  const center: Vec3 = [(min[0] + max[0]) / 2, (bottom + top) / 2, (min[2] + max[2]) / 2];
  const extent = Math.max(max[0] - min[0], top - bottom, max[2] - min[2], 1e-6) * 1.08;
  const light = [0.4, 0.8, 0.45].map((v) => v / Math.hypot(0.4, 0.8, 0.45));

  VIEWS.forEach(({ yaw, pitch }, view) => {
    const offset = view * size;
    const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
    const project = (v: Vec3): Vec3 => {
      const x = v[0] - center[0], y = v[1] - center[1], z = v[2] - center[2];
      const x1 = cy * x - sy * z, z1 = sy * x + cy * z;
      return [
        offset + size / 2 + (x1 / extent) * size,
        size / 2 - ((cp * y - sp * z1) / extent) * size,
        sp * y + cp * z1
      ];
    };

    if (pitch === 0) {
      const ground = Math.round(project([center[0], 0, center[2]])[1]);
      for (let x = offset; x < offset + size && ground >= 0 && ground < size; x++) {
        pixels.set(GROUND, (ground * width + x) * 3);
      }
    }

    for (const t of triangles) {
      const u = [t.b[0] - t.a[0], t.b[1] - t.a[1], t.b[2] - t.a[2]];
      const v = [t.c[0] - t.a[0], t.c[1] - t.a[1], t.c[2] - t.a[2]];
      const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
      const length = Math.hypot(n[0], n[1], n[2]) || 1;
      // both sides of a leaf are lit the same
      const shade = 0.35
        + 0.65 * Math.abs((n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / length);
      const color = (t.instanced ? INSTANCE_COLOR : STEM_COLOR).map((c) =>
        Math.min(255, c * shade)
      );

      const [p0, p1, p2] = [project(t.a), project(t.b), project(t.c)];
      const area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
      if (Math.abs(area) < 1e-9) continue;
      const x0 = Math.max(offset, Math.floor(Math.min(p0[0], p1[0], p2[0])));
      const x1 = Math.min(offset + size - 1, Math.ceil(Math.max(p0[0], p1[0], p2[0])));
      const y0 = Math.max(0, Math.floor(Math.min(p0[1], p1[1], p2[1])));
      const y1 = Math.min(size - 1, Math.ceil(Math.max(p0[1], p1[1], p2[1])));

      for (let y = y0; y <= y1; y++) {
        for (let x = x0; x <= x1; x++) {
          const px = x + 0.5, py = y + 0.5;
          const w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) / area;
          const w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) / area;
          const w2 = 1 - w0 - w1;
          if (w0 < 0 || w1 < 0 || w2 < 0) continue;
          const z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
          const k = y * width + x;
          if (z >= depth[k]) continue;
          depth[k] = z;
          pixels.set(color, k * 3);
        }
      }
    }
  });

  return downsample(pixels, width, size);
}

function downsample(pixels: Uint8Array, width: number, height: number) {
  const w = width / SUPERSAMPLING, h = height / SUPERSAMPLING;
  const out = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      for (let c = 0; c < 3; c++) {
        let sum = 0;
        for (let dy = 0; dy < SUPERSAMPLING; dy++) {
          for (let dx = 0; dx < SUPERSAMPLING; dx++) {
            sum += pixels[((y * SUPERSAMPLING + dy) * width + x * SUPERSAMPLING + dx) * 3 + c];
          }
        }
        out[(y * w + x) * 3 + c] = sum / (SUPERSAMPLING * SUPERSAMPLING);
      }
    }
  }
  return { pixels: out, width: w, height: h };
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(data: Buffer) {
  let c = 0xffffffff;
  for (const byte of data) c = CRC_TABLE[(c ^ byte) & 255] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function pngChunk(type: string, data: Buffer) {
  const body = Buffer.concat([Buffer.from(type), data]);
  const chunk = Buffer.alloc(body.length + 8);
  chunk.writeUInt32BE(data.length, 0);
  body.copy(chunk, 4);
  chunk.writeUInt32BE(crc32(body), body.length + 4);
  return chunk;
}

function encodePng({ pixels, width, height }: ReturnType<typeof downsample>) {
  const rows = Buffer.alloc((width * 3 + 1) * height);
  for (let y = 0; y < height; y++) {
    // every row starts with its filter type, 0 is none
    Buffer.from(pixels.buffer, y * width * 3, width * 3).copy(rows, y * (width * 3 + 1) + 1);
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 2; // rgb
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk('IHDR', header),
    pngChunk('IDAT', deflateSync(rows)),
    pngChunk('IEND', Buffer.alloc(0))
  ]);
}

await mkdir(options.out, { recursive: true });

for (const file of await resolveTargets(targets)) {
  const name = basename(file).replace(/(\.nodarium)?\.json$/, '');
  let result: Int32Array;
  try {
    result = await execute(await loadGraph(file), Number(options.seed));
  } catch (error) {
    // one broken graph shouldn't stop the others
    console.log(
      JSON.stringify({ name, error: error instanceof Error ? error.message : String(error) })
    );
    continue;
  }
  const { triangles, instances } = collectTriangles(result);
  if (triangles.length === 0) {
    console.log(JSON.stringify({ name, error: 'the graph produced no geometry' }));
    continue;
  }

  const out = join(options.out, name + '.png');
  await writeFile(out, encodePng(rasterize(triangles, Number(options.size))));

  const { min, max } = bounds(triangles);
  const round = (v: number[]) => v.map((n) => Math.round(n * 100) / 100);
  console.log(JSON.stringify({
    name,
    screenshot: out,
    triangles: triangles.length,
    instances,
    min: round(min),
    max: round(max),
    height: round([max[1] - Math.min(min[1], 0)])[0],
    width: round([Math.max(max[0] - min[0], max[2] - min[2])])[0]
  }));
}
