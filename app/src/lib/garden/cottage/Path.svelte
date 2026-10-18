<script lang="ts">
  import { T } from '@threlte/core';
  import { BufferGeometry, Float32BufferAttribute, Vector3 } from 'three';
  import { createRandom } from '../random';
  import { loadSurface } from '../textures';
  import { COLORS, GROUND_LEVEL, PATH_WIDTH } from './config';
  import { createPathCurve } from './flora';

  /** world size of one repetition of the brick texture */
  const TEXTURE_SIZE = 1.6;

  /** A ribbon along the path, the bricks run across it */
  function createPathGeometry() {
    const curve = createPathCurve();
    const segments = 160;
    const random = createRandom(5);
    const positions: number[] = [];
    const uvs: number[] = [];
    const indices: number[] = [];
    const side = new Vector3();
    const length = curve.getLength();
    for (let i = 0; i <= segments; i++) {
      const t = i / segments;
      const center = curve.getPointAt(t);
      const tangent = curve.getTangentAt(t);
      side.set(-tangent.z, 0, tangent.x).normalize();
      for (const s of [-1, 1]) {
        // the edges are uneven, the border plants grow over them
        const half = PATH_WIDTH / 2 + (random() - 0.5) * 0.08;
        const p = center.clone().addScaledVector(side, s * half);
        positions.push(p.x, GROUND_LEVEL + 0.03, p.z);
        uvs.push((s * half + PATH_WIDTH / 2) / TEXTURE_SIZE, (t * length) / TEXTURE_SIZE);
      }
      if (i < segments) {
        const a = i * 2;
        indices.push(a, a + 2, a + 1, a + 1, a + 2, a + 3);
      }
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new Float32BufferAttribute(positions, 3));
    geometry.setAttribute('uv', new Float32BufferAttribute(uvs, 2));
    geometry.setIndex(indices);
    geometry.computeVertexNormals();
    return geometry;
  }

  const geometry = createPathGeometry();
  const bricks = loadSurface('bricks');

  $effect(() => () => geometry.dispose());
</script>

<T.Mesh {geometry} receiveShadow>
  <T.MeshStandardMaterial color={COLORS.bricks} {...bricks} />
</T.Mesh>
