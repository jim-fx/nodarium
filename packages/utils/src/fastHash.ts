export function fastHashArrayBuffer(input: string | Int32Array): string {
  const mask = (1n << 64n) - 1n;

  // FNV-1a 64-bit constants
  let h = 0xcbf29ce484222325n; // offset basis
  const FNV_PRIME = 0x100000001b3n;

  // get bytes for string or Int32Array
  let bytes: Uint8Array;
  if (typeof input === 'string') {
    // utf-8 encoding
    bytes = new TextEncoder().encode(input);
  } else {
    // Int32Array -> bytes (little-endian)
    bytes = new Uint8Array(input.length * 4);
    for (let i = 0; i < input.length; i++) {
      const v = input[i] >>> 0; // ensure unsigned 32-bit
      const base = i * 4;
      bytes[base] = v & 0xff;
      bytes[base + 1] = (v >>> 8) & 0xff;
      bytes[base + 2] = (v >>> 16) & 0xff;
      bytes[base + 3] = (v >>> 24) & 0xff;
    }
  }

  // FNV-1a byte-wise
  for (let i = 0; i < bytes.length; i++) {
    h = (h ^ BigInt(bytes[i])) & mask;
    h = (h * FNV_PRIME) & mask;
  }

  // MurmurHash3's fmix64 finalizer (good avalanche)
  h ^= h >> 33n;
  h = (h * 0xff51afd7ed558ccdn) & mask;
  h ^= h >> 33n;
  h = (h * 0xc4ceb9fe1a85ec53n) & mask;
  h ^= h >> 33n;

  // to 16-char hex
  return h.toString(16).padStart(16, '0').slice(-16);
}

export function fastHashString(input: string) {
  if (input.length === 0) return 0;

  let hash = 0;
  for (let i = 0; i < input.length; i++) {
    hash = (hash << 5) - hash + input.charCodeAt(i);
    hash = hash & hash;
  }

  return hash;
}
