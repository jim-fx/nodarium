<script lang="ts">
  import { T } from '@threlte/core';
  import {
    BoxGeometry,
    type BufferGeometry,
    Color,
    Float32BufferAttribute,
    IcosahedronGeometry,
    LatheGeometry,
    MeshStandardMaterial,
    Vector2,
    Vector3
  } from 'three';
  import { mergeVertices } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
  import { createRandom, fbm3 } from '../random';
  import { loadGeneratedTexture } from '../textures';
  import { COLORS, GROUND_LEVEL } from './config';

  /** Pushes the surface in and out, so the clipped shapes look grown instead of modeled */
  function grow(geometry: BufferGeometry, seed: number, amount: number) {
    geometry.deleteAttribute('normal');
    geometry.deleteAttribute('uv');
    const merged = mergeVertices(geometry);
    merged.computeVertexNormals();
    const position = merged.getAttribute('position');
    const normal = merged.getAttribute('normal');
    const p = new Vector3();
    const uvs: number[] = [];
    for (let i = 0; i < position.count; i++) {
      p.fromBufferAttribute(position, i);
      const bump = fbm3(p.x * 0.6, p.y * 0.6, p.z * 0.6, seed, 4) - 0.5
        + (fbm3(p.x * 3, p.y * 3, p.z * 3, seed + 3, 2) - 0.5) * 0.3;
      p.addScaledVector(new Vector3().fromBufferAttribute(normal, i), bump * amount);
      position.setXYZ(i, p.x, p.y, p.z);
      // the texture is projected from the front and the sides
      uvs.push((p.x + p.z * 0.7) / 2.5, p.y / 2.5);
    }
    merged.setAttribute('uv', new Float32BufferAttribute(uvs, 2));
    merged.computeVertexNormals();
    return merged;
  }

  function column(height: number, radius: number, seed: number) {
    const profile: Vector2[] = [];
    for (let i = 0; i <= 24; i++) {
      const t = i / 24;
      // wide in the lower third, pointed at the top like a yew
      const r = radius * Math.sin(Math.PI * Math.pow(t, 0.62)) * (1 - t * 0.25);
      profile.push(new Vector2(Math.max(r, 0.001), t * height));
    }
    return grow(new LatheGeometry(profile, 48), seed, 0.9);
  }

  const map = loadGeneratedTexture('hedge');
  const material = new MeshStandardMaterial({ color: COLORS.hedge, map, roughness: 0.9 });

  const hedge = grow(new BoxGeometry(64, 3.4, 2.6, 256, 14, 10), 4, 0.8);

  const columns = [
    { x: -14.5, z: -10.6, height: 9.5, radius: 2.1 },
    { x: -19.5, z: -8.4, height: 7.2, radius: 2.3 },
    { x: -9.6, z: -13.2, height: 11, radius: 2.4 },
    { x: 12.8, z: -11, height: 10.5, radius: 2.2 },
    { x: 18.2, z: -8.8, height: 8, radius: 2.5 },
    { x: 7.4, z: -13.8, height: 8.5, radius: 2 }
  ].map((c, i) => ({ ...c, geometry: column(c.height, c.radius, 10 + i) }));

  /** Clipped box balls at the entrance of the path */
  const ball = grow(new IcosahedronGeometry(0.75, 5), 2, 0.25);

  /**
   * A line of trees far behind the hedge. They are always out of focus,
   * so a few lumpy canopies in sunlit and shadowed greens are enough.
   */
  const canopy = grow(new IcosahedronGeometry(1, 4), 9, 0.35);
  // clusters of leaves in the light, deep gaps in between
  {
    const position = canopy.getAttribute('position');
    const colors = new Float32Array(position.count * 3);
    for (let i = 0; i < position.count; i++) {
      const n = fbm3(position.getX(i) * 2.5, position.getY(i) * 2.5, position.getZ(i) * 2.5, 17, 4);
      const light = 0.2 + Math.pow(n, 2) * 2.2 + Math.max(position.getY(i), 0) * 0.25;
      colors.fill(light, i * 3, i * 3 + 3);
    }
    canopy.setAttribute('color', new Float32BufferAttribute(colors, 3));
  }
  const random = createRandom(13);
  const trees = Array.from({ length: 26 }, (_, i) => {
    const x = -46 + (i / 25) * 92 + (random() - 0.5) * 3;
    const z = -17 - random() * 10;
    const height = 9 + random() * 9;
    const color = new Color(random() < 0.45 ? COLORS.sunlitTree : COLORS.tree).offsetHSL(
      (random() - 0.5) * 0.04,
      0,
      (random() - 0.5) * 0.08
    );
    return {
      position: [x, GROUND_LEVEL + height * 0.55, z] as [number, number, number],
      scale: [3.5 + random() * 3, height * 0.55, 3 + random() * 2] as [number, number, number],
      rotation: random() * Math.PI,
      material: new MeshStandardMaterial({
        color,
        vertexColors: true,
        roughness: 0.95,
        // the sun shines through the leaves, so even their shadowed side glows a little
        emissive: color.clone().multiplyScalar(0.2)
      })
    };
  });

  $effect(() => () => {
    hedge.dispose();
    ball.dispose();
    canopy.dispose();
    trees.forEach((t) => t.material.dispose());
    columns.forEach((c) => c.geometry.dispose());
    material.dispose();
  });
</script>

<T.Mesh
  geometry={hedge}
  {material}
  position={[0, GROUND_LEVEL + 1.5, -11.2]}
  castShadow
  receiveShadow
/>

{#each columns as c, i (i)}
  <T.Mesh
    geometry={c.geometry}
    {material}
    position={[c.x, GROUND_LEVEL - 0.2, c.z]}
    castShadow
    receiveShadow
  />
{/each}

{#each [-1.9, 2.1] as x (x)}
  <T.Mesh
    geometry={ball}
    {material}
    position={[x, GROUND_LEVEL + 0.55, 6.2]}
    castShadow
    receiveShadow
  />
{/each}

{#each trees as tree, i (i)}
  <T.Mesh
    geometry={canopy}
    material={tree.material}
    position={tree.position}
    scale={tree.scale}
    rotation.y={tree.rotation}
    castShadow
  />
{/each}
