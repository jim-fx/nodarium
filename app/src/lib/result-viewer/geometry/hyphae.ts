import {
  Box3,
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  LineBasicMaterial,
  LineSegments
} from 'three';

export const HYPHAE_TAG = 11;
const HEADER_LENGTH = 2;
const SEGMENT_WORDS = 8;
const BROKEN = 1 << 3;

/** In the order of the tissue numbers of myzel_core */
export const TISSUES = ['mycelium', 'stipe', 'cap', 'gill', 'veil', 'cap cuticle'] as const;
export type Tissue = (typeof TISSUES)[number];

const TISSUE_COLORS: Record<Tissue, Color> = {
  mycelium: new Color('#9ca3af'),
  stipe: new Color('#f1ead8'),
  cap: new Color('#d29b62'),
  gill: new Color('#e88fa0'),
  veil: new Color('#a5b4fc'),
  'cap cuticle': new Color('#8b5a2b')
};

export type HyphaSegment = {
  start: [number, number, number];
  end: [number, number, number];
  radius: number;
  /** -1 for a root */
  parent: number;
  birth: number;
  tissue: Tissue;
  flags: number;
};

/** Reads hyphae `[11, n, ...segments]`, 8 words per segment, parents before their children */
export function readHyphae(data: Int32Array): HyphaSegment[] {
  if (data[0] !== HYPHAE_TAG) {
    throw new Error(`Expected hyphae starting with ${HYPHAE_TAG}, got ${data[0]}`);
  }
  const count = data[1];
  const expected = HEADER_LENGTH + count * SEGMENT_WORDS;
  if (data.length !== expected) {
    throw new Error(`${count} hyphal segments need ${expected} words, got ${data.length}`);
  }
  const floats = new Float32Array(data.buffer, data.byteOffset, data.length);
  const segments: HyphaSegment[] = [];
  for (let i = 0; i < count; i++) {
    const o = HEADER_LENGTH + i * SEGMENT_WORDS;
    const end: [number, number, number] = [floats[o], floats[o + 1], floats[o + 2]];
    const parent = data[o + 4];
    if (parent >= i) {
      throw new Error(`Hyphal segment ${i} grows from ${parent}, parents have to come first`);
    }
    const flags = data[o + 7];
    const start = parent < 0 || flags & BROKEN ? end : segments[parent].end;
    segments.push({
      start,
      end,
      radius: floats[o + 3],
      parent,
      birth: floats[o + 5],
      tissue: TISSUES[data[o + 6]] ?? 'mycelium',
      flags
    });
  }
  return segments;
}

/** Positions and colors of a line per segment, roots and broken segments have no line */
export function hyphaeLines(items: Int32Array[]) {
  const positions: number[] = [];
  const colors: number[] = [];
  for (const item of items) {
    for (const s of readHyphae(item)) {
      if (s.start === s.end) continue;
      const color = TISSUE_COLORS[s.tissue];
      positions.push(...s.start, ...s.end);
      colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
    }
  }
  return { positions, colors };
}

/** Line segments for hyphae, colored by tissue */
export function createHyphaeLines(items: Int32Array[], options: { onTop?: boolean } = {}) {
  const { positions, colors } = hyphaeLines(items);
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new Float32BufferAttribute(positions, 3));
  geometry.setAttribute('color', new Float32BufferAttribute(colors, 3));
  geometry.computeBoundingBox();
  return new LineSegments(
    geometry,
    new LineBasicMaterial({ vertexColors: true, depthTest: !options.onTop })
  );
}

/** Keeps the hyphae of the latest result in the scene */
export function createHyphaePool(parent: Group) {
  const group = new Group();
  parent.add(group);
  let lines: LineSegments | undefined;
  return {
    update(items: Int32Array[]) {
      if (lines) {
        group.remove(lines);
        lines.geometry.dispose();
        (lines.material as LineBasicMaterial).dispose();
        lines = undefined;
      }
      const bounds = new Box3();
      if (items.length === 0) return { totalVertices: 0, totalFaces: 0, bounds };
      lines = createHyphaeLines(items);
      group.add(lines);
      bounds.copy(lines.geometry.boundingBox ?? bounds);
      return { totalVertices: lines.geometry.attributes.position.count, totalFaces: 0, bounds };
    }
  };
}
