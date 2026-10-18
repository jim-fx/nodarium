import {
  BufferAttribute,
  BufferGeometry,
  CylinderGeometry,
  Float32BufferAttribute,
  Matrix4,
  OctahedronGeometry,
  Quaternion,
  Vector3
} from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import { createRandom } from '../random';

/** Builds a grid of quads from a function that maps (u, v) in 0..1 to a position */
function parametric(
  columns: number,
  rows: number,
  point: (u: number, v: number, target: Vector3) => void
) {
  const positions: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];
  const p = new Vector3();
  for (let j = 0; j <= rows; j++) {
    for (let i = 0; i <= columns; i++) {
      point(i / columns, j / rows, p);
      positions.push(p.x, p.y, p.z);
      uvs.push(i / columns, j / rows);
    }
  }
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < columns; i++) {
      const a = j * (columns + 1) + i;
      const b = a + columns + 1;
      indices.push(a, b, a + 1, a + 1, b, b + 1);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new Float32BufferAttribute(positions, 3));
  geometry.setAttribute('uv', new Float32BufferAttribute(uvs, 2));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

/** Shades the geometry from dark at the bottom to bright at the top */
function heightGradient(geometry: BufferGeometry, bottom: number, top = 1) {
  const position = geometry.getAttribute('position');
  geometry.computeBoundingBox();
  const { min, max } = geometry.boundingBox!;
  const colors = new Float32Array(position.count * 3);
  for (let i = 0; i < position.count; i++) {
    const t = (position.getY(i) - min.y) / Math.max(max.y - min.y, 1e-6);
    colors.fill(bottom + (top - bottom) * t, i * 3, i * 3 + 3);
  }
  geometry.setAttribute('color', new BufferAttribute(colors, 3));
  return geometry;
}

/** A single grass blade, one unit high, leaning slightly forward */
export function createBladeGeometry() {
  return heightGradient(
    parametric(1, 4, (u, v, p) => {
      const width = 0.05 * (1 - v * 0.9);
      p.set((u - 0.5) * width, v, v * v * 0.25);
    }),
    0.35
  );
}

/** A leaf one unit long pointing along +y, folded along its midrib */
export function createLeafGeometry() {
  return heightGradient(
    parametric(2, 6, (u, v, p) => {
      const width = Math.sin(Math.PI * Math.pow(v, 0.8)) * 0.24;
      const x = (u - 0.5) * 2 * width;
      p.set(x, v, Math.abs(x) * 0.5 - v * v * 0.25);
    }),
    0.55
  );
}

/** A thin stem, one unit high */
export function createStemGeometry() {
  const geometry = new CylinderGeometry(0.6, 1, 1, 5, 1, true);
  geometry.translate(0, 0.5, 0);
  return geometry;
}

/** Round allium head, a ball of countless tiny star shaped florets */
export function createAlliumHeadGeometry(seed = 1) {
  const random = createRandom(seed);
  const florets: BufferGeometry[] = [];
  const count = 110;
  const golden = Math.PI * (3 - Math.sqrt(5));
  const matrix = new Matrix4();
  const direction = new Vector3();
  const rotation = new Quaternion();
  const up = new Vector3(0, 1, 0);
  for (let i = 0; i < count; i++) {
    const y = 1 - (i / (count - 1)) * 2;
    const r = Math.sqrt(1 - y * y);
    direction.set(Math.cos(golden * i) * r, y, Math.sin(golden * i) * r);
    rotation.setFromUnitVectors(up, direction);
    const distance = 0.85 + random() * 0.2;
    matrix.compose(
      direction.clone().multiplyScalar(distance),
      rotation,
      new Vector3(0.09, 0.2, 0.09)
    );
    florets.push(new OctahedronGeometry(1, 0).applyMatrix4(matrix));
  }
  const head = mergeGeometries(florets);
  head.computeVertexNormals();
  return head;
}

/** A rose bloom, rings of cupped petals that open up towards the outside */
export function createRoseGeometry() {
  const petals: BufferGeometry[] = [];
  const rings = [
    { count: 3, size: 0.4, open: -0.1, radius: 0.05 },
    { count: 4, size: 0.5, open: 0.1, radius: 0.1 },
    { count: 5, size: 0.62, open: 0.35, radius: 0.16 },
    { count: 6, size: 0.72, open: 0.65, radius: 0.24 },
    { count: 7, size: 0.8, open: 1, radius: 0.3 }
  ];
  rings.forEach(({ count, size, open, radius }, ring) => {
    for (let i = 0; i < count; i++) {
      const angle = (i / count) * Math.PI * 2 + ring * 0.7;
      const petal = parametric(4, 4, (u, v, p) => {
        // round petals, widest close to their tip
        const x = (u - 0.5) * size * Math.sin(Math.PI * (0.2 + v * 0.6));
        // cupped across and bent outwards towards the tip
        const bend = open * v;
        p.set(x, Math.cos(bend) * v * size, radius + Math.sin(bend) * v * size - x * x * 1.6);
      });
      petal.rotateY(angle);
      petals.push(petal);
    }
  });
  const bloom = mergeGeometries(petals);
  bloom.computeVertexNormals();
  return heightGradient(bloom, 0.55);
}

/** A small flat flower with round petals, for forget-me-nots and creeping jenny */
export function createFloretGeometry(petalCount = 5) {
  const petals: BufferGeometry[] = [];
  for (let i = 0; i < petalCount; i++) {
    const petal = parametric(3, 3, (u, v, p) => {
      const angle = (u - 0.5) * 1.1;
      const r = 0.15 + v * 0.85;
      p.set(Math.sin(angle) * r * 0.55, r * r * 0.15, Math.cos(angle) * r);
    });
    petal.rotateY((i / petalCount) * Math.PI * 2);
    petals.push(petal);
  }
  const floret = mergeGeometries(petals);
  floret.computeVertexNormals();
  return floret;
}
