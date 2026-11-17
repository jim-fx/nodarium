// The inputs of one execute of a node, as `pnpm profile --dump-args` writes them and
// `pnpm profile:node` reads them: little-endian i32 words [count, length of each input, the
// words of each input]

export function encodeArgs(inputs: Int32Array[]) {
  const words = new Int32Array(1 + inputs.length + inputs.reduce((n, i) => n + i.length, 0));
  words[0] = inputs.length;
  let at = 1 + inputs.length;
  inputs.forEach((input, index) => {
    words[1 + index] = input.length;
    words.set(input, at);
    at += input.length;
  });
  return new Uint8Array(words.buffer);
}

export function decodeArgs(bytes: Uint8Array): Int32Array[] {
  // a small file read by Node shares a pooled buffer, its bytes don't start at 0, and the slice
  // of a Buffer is no copy
  const words = new Int32Array(new Uint8Array(bytes).buffer, 0, bytes.byteLength >> 2);
  const count = words[0];
  if (!(count >= 0) || 1 + count > words.length) throw new Error('not an args file');
  const inputs: Int32Array[] = [];
  let at = 1 + count;
  for (let i = 0; i < count; i++) {
    const length = words[1 + i];
    if (at + length > words.length) throw new Error('not an args file, it ends too early');
    inputs.push(words.slice(at, at + length));
    at += length;
  }
  return inputs;
}

/** `max/myzel/strands` and its second call: `max_myzel_strands.2.args` */
export function argsFileName(type: string, call: number) {
  return `${type.replaceAll('/', '_')}.${call}.args`;
}
