import {
  Box3,
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  LineBasicMaterial,
  LineSegments
} from 'three';

export const LINES_TAG = 12;
const HEADER_LENGTH = 2;
const LINE_WORDS = 7;

/** lowbias32, the hash the Groups material colors its groups by */
function hash(x: number) {
  x ^= x >>> 16;
  x = Math.imul(x, 0x7feb352d);
  x ^= x >>> 15;
  x = Math.imul(x, 0x846ca68b);
  x ^= x >>> 16;
  return x >>> 0;
}

/** The albedo the Groups material gives a group, in linear components */
export function groupColor(group: number) {
  const h = hash(group >>> 0);
  const hue = (h & 0xffff) / 65535;
  const lightness = 0.5 + 0.12 * (((h >>> 16) & 0xff) / 255 - 0.5);
  const saturation = 0.6;
  const channel = (offset: number) => {
    const m = (((hue * 6 + offset) % 6) + 6) % 6;
    const rgb = Math.min(Math.max(Math.abs(m - 3) - 1, 0), 1);
    return lightness + saturation * (rgb - 0.5) * (1 - Math.abs(2 * lightness - 1));
  };
  return new Color(channel(0), channel(4), channel(2));
}

/** Reads Myzel lines `[12, n, ...n × (x0, y0, z0, x1, y1, z1, group)]` */
export function readLines(data: Int32Array) {
  if (data[0] !== LINES_TAG) {
    throw new Error(`Expected lines starting with ${LINES_TAG}, got ${data[0]}`);
  }
  const count = data[1];
  const expected = HEADER_LENGTH + count * LINE_WORDS;
  if (data.length !== expected) {
    throw new Error(`${count} lines need ${expected} words, got ${data.length}`);
  }
  const floats = new Float32Array(data.buffer, data.byteOffset, data.length);
  const positions: number[] = [];
  const groups: number[] = [];
  for (let i = 0; i < count; i++) {
    const o = HEADER_LENGTH + i * LINE_WORDS;
    for (let k = 0; k < 6; k++) positions.push(floats[o + k]);
    groups.push(data[o + 6]);
  }
  return { positions, groups };
}

/** Line segments for paths, colored like their groups in the Groups material */
export function createPathLines(items: Int32Array[], options: { onTop?: boolean } = {}) {
  const positions: number[] = [];
  const colors: number[] = [];
  const colorOf = new Map<number, Color>();
  for (const item of items) {
    const lines = readLines(item);
    positions.push(...lines.positions);
    for (const group of lines.groups) {
      let color = colorOf.get(group);
      if (!color) colorOf.set(group, color = groupColor(group));
      colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new Float32BufferAttribute(positions, 3));
  geometry.setAttribute('color', new Float32BufferAttribute(colors, 3));
  geometry.computeBoundingBox();
  return new LineSegments(
    geometry,
    new LineBasicMaterial({ vertexColors: true, depthTest: !options.onTop })
  );
}

/** Keeps the path lines of the latest result in the scene */
export function createLinesPool(parent: Group) {
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
      lines = createPathLines(items);
      group.add(lines);
      bounds.copy(lines.geometry.boundingBox ?? bounds);
      return { totalVertices: lines.geometry.attributes.position.count, totalFaces: 0, bounds };
    }
  };
}
