import { BufferGeometry, IcosahedronGeometry, Vector3 } from 'three';
import { mergeVertices } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import { fbm3 } from './random';

const vertex = new Vector3();

/** Unit sized boulder, flat on the bottom and lumpy on top */
export function createRockGeometry(seed: number, detail = 5): BufferGeometry {
  const geometry = mergeVertices(new IcosahedronGeometry(1, detail));
  const position = geometry.getAttribute('position');
  for (let i = 0; i < position.count; i++) {
    vertex.fromBufferAttribute(position, i);
    const large = fbm3(vertex.x * 0.9, vertex.y * 0.9, vertex.z * 0.9, seed, 3);
    const small = fbm3(vertex.x * 4, vertex.y * 4, vertex.z * 4, seed + 7, 3);
    vertex.multiplyScalar(0.7 + large * 0.55 + small * 0.08);
    // rocks are buried, the lower half is squashed so they sit firmly on the sand
    if (vertex.y < 0) vertex.y *= 0.35;
    position.setXYZ(i, vertex.x, vertex.y, vertex.z);
  }
  geometry.computeVertexNormals();
  return geometry;
}

/** Flat lumpy cushion, used for moss around the base of the rocks */
export function createMoundGeometry(seed: number): BufferGeometry {
  const geometry = mergeVertices(new IcosahedronGeometry(1, 4));
  const position = geometry.getAttribute('position');
  for (let i = 0; i < position.count; i++) {
    vertex.fromBufferAttribute(position, i);
    const n = fbm3(vertex.x * 1.6, vertex.y * 1.6, vertex.z * 1.6, seed, 4);
    vertex.x *= 0.75 + n * 0.5;
    vertex.z *= 0.75 + n * 0.5;
    vertex.y = vertex.y > 0 ? vertex.y * (0.6 + n * 0.8) : vertex.y * 0.2;
    position.setXYZ(i, vertex.x, vertex.y, vertex.z);
  }
  geometry.computeVertexNormals();
  return geometry;
}
