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

  let index = 2; // Start after the opening bracket
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
  const encoded = [0, 0]; // Initialize encoded array with root bracket notation
  let missingBracketIndex = 1; // Track where to insert the distance to the next bracket

  for (let index = 0; index < array.length; index++) {
    const item = array[index];
    if (Array.isArray(item)) {
      // Update the distance to the next bracket for the last opened bracket
      encoded[missingBracketIndex] = encoded.length - missingBracketIndex;
      if (item.length === 0) {
        // Handle empty arrays by directly adding bracket notation
        encoded.push(0, 1, 1, 1);
      } else {
        // Recursively encode non-empty arrays
        const child = encodeNestedArray(item);
        encoded.push(...child);
      }
      // Update missingBracketIndex to the position of the newly added bracket
      missingBracketIndex = encoded.length - 1;
    } else {
      // Handle non-array items
      encoded.push(item);
      // Update the distance for the last opened bracket
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
  let nextBracketIndex = dense[index + 1] + index + 1; // Calculate the index of the next bracket

  index += 2; // Skip the initial bracket notation
  while (index < dense.length) {
    if (index === nextBracketIndex) {
      if (dense[index] === 0) {
        // Opening bracket detected
        const [p, nextIndex, _nextBracketIndex] = decode_recursive(
          dense,
          index
        );
        decoded.push(p as number[]);
        index = nextIndex + 1;
        nextBracketIndex = _nextBracketIndex;
      } else {
        // Closing bracket detected
        nextBracketIndex = dense[index + 1] + index + 1;
        return [decoded, index, nextBracketIndex] as const;
      }
    } else if (index < nextBracketIndex) {
      decoded.push(dense[index]); // Add regular number to decoded array
    }
    index++;
  }
  return [decoded, index, nextBracketIndex] as const;
}

export function decodeNestedArray(dense: number[] | Int32Array) {
  return decode_recursive(dense, 0)[0];
}

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
        // if opening bracket
        argStartIndex = index + 2;
      }

      if (depth === 0 && value === 1) {
        // if closing bracket
        res.push(input.slice(argStartIndex, index));
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
