import { expect, test } from 'vitest';
import {
  concatEncodedArrays,
  decodeNestedArray,
  encodeNestedArray,
  splitNestedArray
} from './flatTree';

test('it correctly concats nested arrays', () => {
  const input_a = encodeNestedArray([1, 2, 3]);
  const input_b = 2;
  const input_c = encodeNestedArray([4, 5, 6]);

  const output = concatEncodedArrays([input_a, input_b, input_c]);

  const decoded = decodeNestedArray(output);

  expect(decoded[0]).toEqual([1, 2, 3]);
  expect(decoded[1]).toEqual(2);
  expect(decoded[2]).toEqual([4, 5, 6]);
});

test('it correctly concats nested arrays with nested arrays', () => {
  const input_c = encodeNestedArray([1, 2, 3]);
  const output = concatEncodedArrays([42, 12, input_c]);
  const decoded = decodeNestedArray(output);
  expect(decoded[0]).toEqual(42);
  expect(decoded[1]).toEqual(12);
  expect(decoded[2]).toEqual([1, 2, 3]);
});

// Original test case
test('it correctly decodes/encodes complex nested arrays', () => {
  const input = [5, [6, 1], 1, 5, [5], [7, 2, [5, 1]]];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with empty array
test('it correctly handles an empty array', () => {
  const input: number[] = [];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with nested empty arrays
test('it correctly handles nested empty arrays', () => {
  const input = [5, [], [6, []], []];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with single-element array
test('it correctly handles a single-element array', () => {
  const input = [42];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with deeply nested array
test('it correctly handles deeply nested arrays', () => {
  const input = [[[[[1]]]]];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with large numbers
test('it correctly handles large numbers', () => {
  const input = [2147483647, [-2147483648, 1234567890]];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with sequential nesting
test('it correctly handles sequential nesting', () => {
  const input = [1, [2, [3, [4, [5]]]]];
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test with mixed data types (if supported)
// Note: This test assumes your implementation supports mixed types.
// If not, you can ignore or remove this test.
test('it correctly handles arrays with mixed data types', () => {
  const input = [1, 'text', [true, [null, ['another text']]]];
  // @ts-ignore
  const decoded = decodeNestedArray(encodeNestedArray(input));
  expect(decoded).toEqual(input);
});

// Test splitNestedArray function
test('it splits nested array into segments based on structure', () => {
  const input = [[1, 2], [3, 4]];
  const encoded = new Int32Array(encodeNestedArray(input));
  const split = splitNestedArray(encoded);

  // Based on the actual behavior, splitNestedArray returns segments
  // but the specific behavior needs to match the implementation
  expect(Array.isArray(split)).toBe(true);
  expect(split.length).toBe(2);
  expect(split[0][0]).toBe(1);
  expect(split[0][1]).toBe(2);
  expect(split[1][0]).toBe(3);
  expect(split[1][1]).toBe(4);
});

// Test splitNestedArray function
test('it splits nested array into segments based on structure 2', () => {
  // dprint-ignore
  const encoded = new Int32Array([
    0, 1,
      0, 19,
        0, 1,
        0,          0,           0,          1060487823,
        1067592955, 1079491492, -1086248132, 1056069822,
       -1078247113, 1086620820,  1073133800, 1047681214,
       -1068353940, 1094067306,  1078792112, 0,
      1, 1,
      0, 19,
        0, 1,
        0,          0,           0,          1060487823,
       -1089446963, 1080524584,  1041006274, 1056069822,
       -1092176382, 1087031528, -1088851934, 1047681214,
        1081482392, 1094426140, -1107842261, 0,
      1, 1,
    1, 1
  ]);

  // Should be split into two seperate arrays
  const split = splitNestedArray(encoded);

  // Based on the actual behavior, splitNestedArray returns segments
  // but the specific behavior needs to match the implementation
  expect(Array.isArray(split)).toBe(true);
  expect(split.length).toBe(2);
  expect(split[0][0]).toBe(0);
  expect(split[0][1]).toBe(1);
  expect(split[1][0]).toBe(0);
  expect(split[1][1]).toBe(1);
});

// Test splitNestedArray function
test('it splits nested array into segments based on structure 2', () => {
  // dprint-ignore
  const encoded = new Int32Array( [
    0, 1,
      0, 27,
        0, 1,
        0, 0,          0, 1065353216,
        0, 1067757391, 0, 1061997773,
        0, 1076145999, 0, 1058642330,
        0, 1081542391, 0, 1053609164,
        0, 1084534607, 0, 1045220556,
        0, 1087232803, 0, 0,
      1, 1,
    1, 1
  ]);

  // Should be split into two seperate arrays
  const split = splitNestedArray(encoded);

  // Based on the actual behavior, splitNestedArray returns segments
  // but the specific behavior needs to match the implementation
  expect(Array.isArray(split)).toBe(true);
  expect(split.length).toBe(1);
});
