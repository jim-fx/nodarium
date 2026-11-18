/** The type word of a pattern, see `nodarium_utils::pattern` */
export const PATTERN_TAG = 15;
const DISC = 0;
const HEADER_WORDS = 6;
/** A ring is drawn with at most this many points */
const PATTERN_SEGMENTS = 2048;

type Pattern = { inner: number; outer: number; rings: Float32Array[] };

/** The words inside the brackets of `[0, n + 1, ...words, 1, 1]` */
function unframe(words: Int32Array) {
  const n = words.length;
  const framed = n >= 4 && words[0] === 0 && words[1] === n - 3 && words[n - 2] === 1
    && words[n - 1] === 1;
  return framed ? words.subarray(2, n - 2) : undefined;
}

/** A pattern as a node sends it, framed, or the one item of a list. Undefined for anything else. */
export function decodePattern(words: Int32Array): Pattern | undefined {
  if (words[0] !== PATTERN_TAG) {
    const n = words.length;
    if (n >= 8 && words[0] === 0 && words[1] === 1 && words[n - 2] === 1 && words[n - 1] === 1) {
      const inner = unframe(words.subarray(2, n - 2));
      const pattern = inner && decodePattern(inner);
      if (pattern) return pattern;
    }
    const inner = unframe(words);
    return inner && decodePattern(inner);
  }
  if (words.length < HEADER_WORDS || words[1] !== DISC || words[2] < 0) return;
  const rings = words[2];
  const counts = words.subarray(HEADER_WORDS, HEADER_WORDS + rings);
  if (counts.length !== rings || counts.some((c) => c < 0)) return;
  const total = counts.reduce((sum, c) => sum + c, 0);
  if (words.length !== HEADER_WORDS + rings + total) return;
  const floats = new Float32Array(words.buffer, words.byteOffset, words.length);
  let offset = HEADER_WORDS + rings;
  return {
    inner: floats[3],
    outer: floats[4],
    rings: [...counts].map((c) => {
      const ring = floats.slice(offset, offset + c);
      offset += c;
      return ring;
    })
  };
}

function radius(pattern: Pattern, ring: number) {
  const n = pattern.rings.length;
  return n < 2 ? pattern.inner : pattern.inner + (pattern.outer - pattern.inner) * ring / (n - 1);
}

/** The value of a ring at an angle, linear between the two values around it */
function ringValue(values: Float32Array, angle: number) {
  const n = values.length;
  if (!n) return 0;
  const turn = Math.PI * 2;
  const x = (((angle % turn) + turn) % turn) / turn * n - 0.5;
  const x0 = Math.floor(x);
  const t = x - x0;
  const j0 = ((x0 % n) + n) % n;
  return values[j0] * (1 - t) + values[(j0 + 1) % n] * t;
}

const byte = (v: number) => Math.round(Math.min(Math.max(v, 0), 1) * 255);

/**
 * A pattern as a flat disc around the y axis, `[0, 1, geometry, 1, 1]` like a result: dark where
 * its values are low, light where they are high. Its angles count around the axis like the rays
 * of `cap` on an upright stipe. Undefined for words that are no pattern.
 */
export function patternPreview(words: Int32Array) {
  const pattern = decodePattern(words);
  if (!pattern) return;
  const rings = pattern.rings.length;
  const longest = Math.max(0, ...pattern.rings.map((r) => r.length));
  const segments = Math.min(Math.max(longest, 3), PATTERN_SEGMENTS);
  const v = rings < 2 ? 0 : rings * segments;
  const f = rings < 2 ? 0 : (rings - 1) * segments * 2;
  // the brackets of the list, of the geometry and its header, faces, positions, normals, colors,
  // groups, and the two closing brackets
  const length = 2 + 7 + f * 3 + v * 8 + 2 + 2;
  const out = new Int32Array(length);
  const floats = new Float32Array(out.buffer);
  out.set([0, 1, 0, length - 7, 1, v, f, -1, 1 | 4]);
  const faces = 9;
  const positions = faces + f * 3;
  const normals = positions + v * 3;
  const colors = normals + v * 3;
  if (v) {
    for (let i = 0; i < rings; i++) {
      const r = radius(pattern, i);
      for (let j = 0; j < segments; j++) {
        const k = i * segments + j;
        // on the middle of the values, like the pattern places them
        const angle = Math.PI * 2 * (j + 0.5) / segments;
        floats.set([-Math.sin(angle) * r, 0, -Math.cos(angle) * r], positions + k * 3);
        floats.set([0, 1, 0], normals + k * 3);
        const value = 0.15 + 0.8 * Math.min(Math.max(ringValue(pattern.rings[i], angle), 0), 1);
        out[colors + k] = byte(value) | byte(value * 0.92) << 8 | byte(value * 0.85) << 16
          | 255 << 24;
      }
    }
    const vertex = (i: number, j: number) => i * segments + j % segments;
    let at = faces;
    for (let i = 0; i < rings - 1; i++) {
      for (let j = 0; j < segments; j++) {
        // counter-clockwise seen from above
        out.set([vertex(i, j), vertex(i + 1, j), vertex(i + 1, j + 1)], at);
        out.set([vertex(i, j), vertex(i + 1, j + 1), vertex(i, j + 1)], at + 3);
        at += 6;
      }
    }
  }
  out.set([1, 1, 1, 1], length - 4);
  return out;
}
