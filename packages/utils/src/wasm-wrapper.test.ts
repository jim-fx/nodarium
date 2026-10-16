import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { concatEncodedArrays } from './flatTree';
import { createWasmWrapper, isWasmSlice, readNodeValue } from './wasm-wrapper';

// see fixtures/*.wat for what these modules do
function load(name: string) {
  const buffer = readFileSync(new URL(`./fixtures/${name}.wasm`, import.meta.url));
  return createWasmWrapper(buffer as unknown as ArrayBuffer);
}

describe('createWasmWrapper with the ABI v1', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('reads the definition from the custom section', () => {
    expect(load('v1').get_definition()).toEqual({
      id: 'test/abi/v1',
      outputs: ['float'],
      inputs: {}
    });
  });

  it('passes every input and returns the result inside wasm memory', () => {
    const node = load('v1');
    node.reset();

    const result = node.execute([Int32Array.of(7), Int32Array.of(1, 2, 3), new Int32Array()]);

    expect(isWasmSlice(result)).toBe(true);
    expect(readNodeValue(result)).toEqual(Int32Array.of(3, 7, 1, 2, 3));
  });

  it('copies results of other modules directly', () => {
    const producer = load('v1');
    const consumer = load('v1');
    producer.reset();
    consumer.reset();

    const produced = producer.execute([Int32Array.of(4, 5)]);
    const result = consumer.execute([produced]);

    // the producer returned [1, 4, 5]
    expect(readNodeValue(result)).toEqual(Int32Array.of(1, 1, 4, 5));
  });

  it('accepts results of the same module as inputs', () => {
    const node = load('v1');
    node.reset();

    const first = node.execute([Int32Array.of(4, 5)]);
    const second = node.execute([first, first]);

    expect(readNodeValue(second)).toEqual(Int32Array.of(2, 1, 4, 5, 1, 4, 5));
  });

  it('handles inputs and results that grow the memory', () => {
    const node = load('v1');
    node.reset();
    // 1MB, far more than the module's initial 64KB, the memory grows while
    // the input is allocated and again for the result
    const big = new Int32Array(256 * 1024).map((_, i) => i);

    const result = node.execute([big]);

    const value = readNodeValue(result);
    expect(value.length).toBe(big.length + 1);
    expect(value[0]).toBe(1);
    expect(value.subarray(1)).toEqual(big);
  });

  it('returns copies that survive a reset', () => {
    const node = load('v1');
    node.reset();
    const copy = readNodeValue(node.execute([Int32Array.of(9)]));

    node.reset();
    node.execute([Int32Array.of(1, 1, 1)]);

    expect(copy).toEqual(Int32Array.of(1, 9));
  });

  it('reports panics before trapping', () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const node = load('v1');
    node.reset();

    expect(() => node.execute([])).toThrow(WebAssembly.RuntimeError);
    expect(error).toHaveBeenCalledWith('RUST PANIC:', 'no inputs');
  });
});

describe('createWasmWrapper with the legacy ABI', () => {
  it('concatenates the inputs and copies the result out', () => {
    const node = load('legacy');

    const result = node.execute([Int32Array.of(7), Int32Array.of(1, 2, 3)]);

    expect(isWasmSlice(result)).toBe(false);
    // single words are inlined as plain numbers
    expect(result).toEqual(concatEncodedArrays([7, [1, 2, 3]]));
  });

  it('accepts results of v1 modules as inputs', () => {
    const producer = load('v1');
    const node = load('legacy');
    producer.reset();

    const result = node.execute([producer.execute([Int32Array.of(4, 5)])]);

    expect(result).toEqual(concatEncodedArrays([[1, 4, 5]]));
  });

  it('ignores reset', () => {
    const node = load('legacy');
    expect(() => node.reset()).not.toThrow();
  });
});
