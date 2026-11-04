type SparseArray<T = number> = (T | T[] | SparseArray<T>)[];

export function concatEncodedArrays(
  input: (number | number[] | Int32Array)[]
): Int32Array {
  let totalLength = 4;
  for (let i = 0; i < input.length; i++) {
    const item = input[i];
    if (Array.isArray(item) || item instanceof Int32Array) {
      totalLength += item.length;
    } else {
      totalLength++;
    }
  }

  const result = new Int32Array(totalLength);

  result[0] = 0;
  result[1] = 1;

  let index = 2;
  let last_closing_bracket = 1;

  for (let i = 0; i < input.length; i++) {
    const item = input[i];
    if (Array.isArray(item) || item instanceof Int32Array) {
      result.set(item, index);
      index += item.length;
      last_closing_bracket = index - 1;
    } else {
      result[last_closing_bracket]++;
      result[index] = item;
      index++;
    }
  }

  result[totalLength - 2] = 1;
  result[totalLength - 1] = 1;

  return result;
}

// Encodes a nested array into a flat array with bracket and distance notation
export function encodeNestedArray(array: SparseArray): number[] {
  const encoded = [0, 0];
  let missingBracketIndex = 1; // Track where to insert the distance to the next bracket

  for (let index = 0; index < array.length; index++) {
    const item = array[index];
    if (Array.isArray(item)) {
      encoded[missingBracketIndex] = encoded.length - missingBracketIndex;
      if (item.length === 0) {
        encoded.push(0, 1, 1, 1);
      } else {
        const child = encodeNestedArray(item);
        encoded.push(...child);
      }
      missingBracketIndex = encoded.length - 1;
    } else {
      encoded.push(item);
      if (missingBracketIndex) encoded[missingBracketIndex] = index + 2;
    }
  }

  return [...encoded, 1, 1];
}

function decode_recursive(dense: number[] | Int32Array, index = 0) {
  if (dense instanceof Int32Array) {
    dense = Array.from(dense);
  }

  const decoded: (number | number[])[] = [];
  let nextBracketIndex = dense[index + 1] + index + 1;

  index += 2;
  while (index < dense.length) {
    if (index === nextBracketIndex) {
      if (dense[index] === 0) {
        const [p, nextIndex, _nextBracketIndex] = decode_recursive(
          dense,
          index
        );
        decoded.push(p as number[]);
        index = nextIndex + 1;
        nextBracketIndex = _nextBracketIndex;
      } else {
        nextBracketIndex = dense[index + 1] + index + 1;
        return [decoded, index, nextBracketIndex] as const;
      }
    } else if (index < nextBracketIndex) {
      decoded.push(dense[index]);
    }
    index++;
  }
  return [decoded, index, nextBracketIndex] as const;
}

export function decodeNestedArray(dense: number[] | Int32Array) {
  return decode_recursive(dense, 0)[0];
}

/** The items are views into `input`, copy them before `input` is reused or transferred */
export function splitNestedArray(input: Int32Array) {
  let index = 0;
  const length = input.length;
  let res: Int32Array[] = [];

  let nextBracketIndex = 0;
  let argStartIndex = 0;
  let depth = -1;

  while (index < length) {
    const value = input[index];

    if (index === nextBracketIndex) {
      nextBracketIndex = index + input[index + 1] + 1;
      if (value === 0) {
        depth++;
      } else {
        depth--;
      }

      if (depth === 1 && value === 0) {
        argStartIndex = index + 2;
      }

      if (depth === 0 && value === 1) {
        res.push(input.subarray(argStartIndex, index));
        argStartIndex = index + 2;
      }

      index = nextBracketIndex;
      continue;
    }

    // we should not be here

    index++;
  }

  return res;
}
