<script lang="ts">
  import { T } from '@threlte/core';
  import {
    BufferAttribute,
    DataTexture,
    LinearFilter,
    LinearMipmapLinearFilter,
    Path,
    RedFormat,
    Shape,
    ShapeGeometry
  } from 'three';
  import { createBedBorder } from '../bed-border';
  import { BED_DEPTH, BED_WIDTH } from '../config';
  import type { Island } from '../layout';
  import { loadSurface } from '../textures';
  import { COLORS, FLOOR_LEVEL } from './config';
  import { pondOutline } from './pond';
  import { renderRakePattern } from './rake-pattern';

  type Props = { islands: Island[]; onground: () => void };
  const { islands, onground }: Props = $props();

  /** The forest floor around the bed, it fades into the fog */
  const FLOOR_SIZE = 70;

  const BORDER_THICKNESS = 0.32;
  const BORDER_TOP = 0.1;

  function createSandGeometry() {
    const shape = new Shape();
    shape.moveTo(-BED_WIDTH / 2, -BED_DEPTH / 2);
    shape.lineTo(BED_WIDTH / 2, -BED_DEPTH / 2);
    shape.lineTo(BED_WIDTH / 2, BED_DEPTH / 2);
    shape.lineTo(-BED_WIDTH / 2, BED_DEPTH / 2);

    // the mesh is rotated onto the ground, so shape y becomes -z
    const hole = new Path();
    pondOutline().forEach(({ x, z }, i) => (i ? hole.lineTo(x, -z) : hole.moveTo(x, -z)));
    shape.holes.push(hole);

    const geometry = new ShapeGeometry(shape);
    const position = geometry.getAttribute('position');
    const uv = geometry.getAttribute('uv') as BufferAttribute;
    for (let i = 0; i < position.count; i++) {
      uv.setXY(
        i,
        (position.getX(i) + BED_WIDTH / 2) / BED_WIDTH,
        (position.getY(i) + BED_DEPTH / 2) / BED_DEPTH
      );
    }
    return geometry;
  }

  const sandGeometry = createSandGeometry();
  const gravel = loadSurface('gravel');
  for (const texture of [gravel.map, gravel.aoMap]) {
    texture.repeat.set(BED_WIDTH / 2.5, BED_DEPTH / 2.5);
  }
  const planks = loadSurface('planks');
  const forest = loadSurface('forest', FLOOR_SIZE / 3);

  const rakeTexture = $derived.by(() => {
    const { data, width, height } = renderRakePattern(islands);
    const texture = new DataTexture(data, width, height, RedFormat);
    texture.minFilter = LinearMipmapLinearFilter;
    texture.magFilter = LinearFilter;
    texture.generateMipmaps = true;
    texture.needsUpdate = true;
    return texture;
  });

  $effect(() => {
    const texture = rakeTexture;
    return () => texture.dispose();
  });

  const beams = createBedBorder({
    top: BORDER_TOP,
    bottom: FLOOR_LEVEL,
    thickness: BORDER_THICKNESS,
    textureSize: 2.4
  });
</script>

<T.Mesh geometry={sandGeometry} rotation.x={-Math.PI / 2} receiveShadow onclick={onground}>
  <T.MeshStandardMaterial
    color={COLORS.sand}
    {...gravel}
    bumpMap={rakeTexture}
    bumpScale={3}
  />
</T.Mesh>

{#each beams as beam, i (i)}
  <T.Mesh geometry={beam.geometry} position={beam.position} castShadow receiveShadow>
    <T.MeshStandardMaterial color={COLORS.wood} {...planks} />
  </T.Mesh>
{/each}

<T.Mesh rotation.x={-Math.PI / 2} position.y={FLOOR_LEVEL} receiveShadow onclick={onground}>
  <T.CircleGeometry args={[FLOOR_SIZE / 2, 64]} />
  <T.MeshStandardMaterial color={COLORS.floor} {...forest} />
</T.Mesh>
