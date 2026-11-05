import { HYPHAE_TAG } from './hyphae';

type Segment = { end: number[]; parent: number; tissue?: number; flags?: number; birth?: number };

/** [11, n, ...x y z radius parent birth tissue flags] */
export function encodeHyphae(segments: Segment[]) {
  const data = new Int32Array(2 + segments.length * 8);
  const floats = new Float32Array(data.buffer);
  data.set([HYPHAE_TAG, segments.length]);
  segments.forEach((s, i) => {
    const o = 2 + i * 8;
    floats.set([...s.end, 0.001], o);
    data[o + 4] = s.parent;
    floats[o + 5] = s.birth ?? i;
    data[o + 6] = s.tissue ?? 0;
    data[o + 7] = s.flags ?? 0;
  });
  return data;
}
