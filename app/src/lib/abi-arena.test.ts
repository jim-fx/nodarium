// The arena of `#[nodarium_execute(arena)]` lays its buffers in pages it grows itself, so the Rust
// allocator grows the memory before and after them. It must never hand out the arena's pages:
// the compiled `output` node takes a large list of paths into the arena and builds its lines and
// their list with the allocator, which has to grow the memory past the arena. See docs/ABI.md.
// `set_mesh_color` paints the mesh the host copied in where it is, and lengthens it in the arena
// when it has no colors yet.
import type { WasmSlice } from '@nodarium/types';
import { createWasmWrapper } from '@nodarium/utils';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

type Exports = {
  memory: WebAssembly.Memory;
  nodarium_alloc: (bytes: number) => number;
  nodarium_reset: () => void;
  nodarium_execute: (argsPtr: number, argc: number) => number;
};

const ON_CI = !!(process.env.CI || process.env.GITEA_ACTIONS);
const NODE = resolve('static/nodes/max/myzel/output.wasm');
const PAINT = resolve('static/nodes/max/myzel/set_mesh_color.wasm');
const BUILT = existsSync(NODE) && existsSync(PAINT);

const LINES_TAG = 12;
const VICTIM_WORDS = 64 * 1024;

function instantiate(path = NODE) {
  let panic: [number, number] | undefined;
  const env = {
    nodarium_panic: (ptr: number, len: number) => (panic = [ptr, len]),
    nodarium_log: () => {}
  };
  const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(path)), { env });
  const exports = instance.exports as unknown as Exports;
  const message = () =>
    panic && new TextDecoder().decode(new Uint8Array(exports.memory.buffer, ...panic));
  return { exports, panic: message };
}

/** One path of `points` points along x, as `[0, 1, path, 1, 1]` */
function pathList(points: number) {
  const words = new Int32Array(4 + 9 + points * 4 + 2);
  words.set([0, 1, 0, points * 4 + 8, 0, 1, 7, -1, 0, 3, 0]);
  const floats = new Float32Array(words.buffer, 11 * 4, points * 4);
  for (let i = 0; i < points; i++) floats.set([i * 0.001, 1, 2, 0.1], i * 4);
  words.set([1, 1, 1, 1], words.length - 4);
  return words;
}

const marker = (i: number) => 0x5a5a0000 ^ i;

/** The first word of `view` that isn't `word` of its index, -1 for none */
const changed = (view: Int32Array, word: (i: number) => number) =>
  view.findIndex((w, i) => w !== word(i));

describe.skipIf(!BUILT && !ON_CI)('the arena of a node', () => {
  it('is never handed out by the allocator when it grows the memory after it', () => {
    const { exports, panic } = instantiate();
    const memory = exports.memory;
    const before = memory.buffer.byteLength;

    // each run larger than the memory the allocator got back from the ones before
    for (const [run, points] of [100_000, 250_000, 600_000].entries()) {
      exports.nodarium_reset();
      const input = pathList(points);
      const inputPtr = exports.nodarium_alloc(input.byteLength);
      new Int32Array(memory.buffer, inputPtr, input.length).set(input);
      const tablePtr = exports.nodarium_alloc(8);
      new Uint32Array(memory.buffer, tablePtr, 2).set([inputPtr, input.length]);
      // the last buffer of the run, at the tail of the arena
      const victimPtr = exports.nodarium_alloc(VICTIM_WORDS * 4);
      new Int32Array(memory.buffer, victimPtr, VICTIM_WORDS).set(
        Int32Array.from({ length: VICTIM_WORDS }, (_, i) => marker(i))
      );
      const arenaEnd = memory.buffer.byteLength;
      if (run === 0) {
        expect(inputPtr).toBeGreaterThanOrEqual(before);
        expect(victimPtr + VICTIM_WORDS * 4).toBeLessThanOrEqual(arenaEnd);
      }

      let resultPtr: number;
      try {
        resultPtr = exports.nodarium_execute(tablePtr, 1);
      } catch (error) {
        throw new Error(`output panicked: ${panic()}`, { cause: error });
      }

      expect(memory.buffer.byteLength, 'the allocator grew the memory after the arena')
        .toBeGreaterThan(arenaEnd);
      const victim = new Int32Array(memory.buffer, victimPtr, VICTIM_WORDS);
      expect(changed(victim, marker), `run ${run}: the arena's last buffer changed`).toBe(-1);
      const kept = new Int32Array(memory.buffer, inputPtr, input.length);
      expect(changed(kept, (i) => input[i]), `run ${run}: the input changed`).toBe(-1);

      const [ptr, len] = new Uint32Array(memory.buffer, resultPtr, 2);
      for (const [start, bytes] of [[inputPtr, input.byteLength], [victimPtr, VICTIM_WORDS * 4]]) {
        expect(ptr + len * 4 <= start || ptr >= start + bytes, 'the result lies outside the arena')
          .toBe(true);
      }
      const result = new Int32Array(memory.buffer, ptr, len);
      expect([result[4], result[5]]).toEqual([LINES_TAG, points - 1]);
      const last = new Float32Array(memory.buffer, ptr + (len - 4 - 7) * 4, 6);
      expect(last[3]).toBeCloseTo((points - 1) * 0.001, 3);
    }
  });
});

const MESH_TAG = 14;
const COLORS = 1;

/** A strip of `v` vertices, with one color per vertex when `colored` */
function mesh(v: number, colored: boolean) {
  const f = v - 2;
  const words = new Int32Array(4 + f * 3 + v * (colored ? 8 : 7));
  words.set([MESH_TAG, v, f, colored ? COLORS : 0]);
  for (let i = 0; i < f; i++) words.set([i, i + 1, i + 2], 4 + i * 3);
  const vertices = 4 + f * 3;
  new Float32Array(words.buffer, vertices * 4, v * 6).set(
    Float32Array.from({ length: v * 6 }, (_, i) => i * 0.5)
  );
  for (let i = 0; i < v; i++) {
    if (colored) words[vertices + v * 6 + i] = i;
    words[words.length - v + i] = i % 5;
  }
  return words;
}

/** What painting `words` in `[r, g, b]` gives */
function painted(words: Int32Array, [r, g, b]: number[]) {
  const [, v, f, flags] = words;
  const colors = 4 + f * 3 + v * 6;
  const out = new Int32Array(colors + v * 2);
  out.set(words.subarray(0, colors));
  out[3] = COLORS;
  out.fill(r | g << 8 | b << 16 | 0xff000000, colors, colors + v);
  out.set(words.subarray(flags & COLORS ? colors + v : colors), colors + v);
  return out;
}

const color = (rgb: number[]) => Int32Array.of(0, 4, ...rgb, 1, 1);

describe.skipIf(!BUILT && !ON_CI)('set_mesh_color', () => {
  it('paints a mesh the host copied in where it is', () => {
    const { exports, panic } = instantiate(PAINT);
    const memory = exports.memory;
    for (
      const [run, [v, colored]] of [[50_000, false], [60_000, true], [80_000, false]].entries()
    ) {
      exports.nodarium_reset();
      const input = mesh(v as number, colored as boolean);
      const inputPtr = exports.nodarium_alloc(input.byteLength);
      new Int32Array(memory.buffer, inputPtr, input.length).set(input);
      const rgb = color([run, 2, 3]);
      const colorPtr = exports.nodarium_alloc(rgb.byteLength);
      new Int32Array(memory.buffer, colorPtr, rgb.length).set(rgb);
      const tablePtr = exports.nodarium_alloc(16);
      new Uint32Array(memory.buffer, tablePtr, 4).set([
        inputPtr,
        input.length,
        colorPtr,
        rgb.length
      ]);

      let resultPtr: number;
      try {
        resultPtr = exports.nodarium_execute(tablePtr, 2);
      } catch (error) {
        throw new Error(`set_mesh_color panicked: ${panic()}`, { cause: error });
      }
      const [ptr, len] = new Uint32Array(memory.buffer, resultPtr, 2);
      expect(ptr, `run ${run}: painted where the input is`).toBe(inputPtr);
      const expected = painted(input, [run, 2, 3]);
      expect(len).toBe(expected.length);
      const result = new Int32Array(memory.buffer, ptr, len);
      expect(changed(result, (i) => expected[i]), `run ${run}: the painted mesh`).toBe(-1);
    }
  });

  it('holds a mesh once, and reads the length of the longer result', () => {
    const wasm = createWasmWrapper(readFileSync(PAINT) as unknown as ArrayBuffer);
    const input = mesh(200_000, false);
    wasm.reset();
    const result = wasm.execute([input, color([9, 8, 7])]) as WasmSlice;
    const expected = painted(input, [9, 8, 7]);
    expect(result.len).toBe(expected.length);
    const words = new Int32Array(result.memory.buffer, result.ptr, result.len);
    expect(changed(words, (i) => expected[i])).toBe(-1);
    expect(result.memory.buffer.byteLength).toBeLessThan(expected.byteLength * 1.5);
  });

  it('paints its own result, which other nodes may still read, into a new mesh', () => {
    const wasm = createWasmWrapper(readFileSync(PAINT) as unknown as ArrayBuffer);
    const input = mesh(50_000, false);
    wasm.reset();
    const first = wasm.execute([input, color([1, 2, 3])]) as WasmSlice;
    const before = new Int32Array(first.memory.buffer, first.ptr, first.len).slice();
    const second = wasm.execute([first, color([4, 5, 6])]) as WasmSlice;
    expect(second.ptr).not.toBe(first.ptr);
    const kept = new Int32Array(first.memory.buffer, first.ptr, first.len);
    expect(changed(kept, (i) => before[i]), 'the first result changed').toBe(-1);
    const expected = painted(input, [4, 5, 6]);
    const words = new Int32Array(second.memory.buffer, second.ptr, second.len);
    expect(changed(words, (i) => expected[i])).toBe(-1);
  });
});
