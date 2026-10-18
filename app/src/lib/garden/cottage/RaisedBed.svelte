<script lang="ts">
  import { T } from '@threlte/core';
  import { createBedBorder } from '../bed-border';
  import { BED_DEPTH, BED_WIDTH } from '../config';
  import { loadSurface } from '../textures';
  import { COLORS, GROUND_LEVEL } from './config';

  type Props = { onground: () => void };
  const { onground }: Props = $props();

  /** The lawn under the grass blades, it fades into the fog */
  const LAWN_SIZE = 90;

  const soil = loadSurface('soil');
  for (const texture of [soil.map, soil.normalMap!, soil.aoMap]) {
    texture.repeat.set(BED_WIDTH / 3, BED_DEPTH / 3);
  }
  const boards = loadSurface('boards');
  const lawn = loadSurface('lawn', LAWN_SIZE / 4);

  const beams = createBedBorder({
    top: 0.2,
    bottom: GROUND_LEVEL - 0.05,
    thickness: 0.28,
    textureSize: 3
  });
</script>

<T.Mesh rotation.x={-Math.PI / 2} receiveShadow onclick={onground}>
  <T.PlaneGeometry args={[BED_WIDTH, BED_DEPTH]} />
  <T.MeshStandardMaterial color={COLORS.soil} {...soil} />
</T.Mesh>

{#each beams as beam, i (i)}
  <T.Mesh geometry={beam.geometry} position={beam.position} castShadow receiveShadow>
    <T.MeshStandardMaterial color={COLORS.boards} {...boards} />
  </T.Mesh>
{/each}

<T.Mesh rotation.x={-Math.PI / 2} position.y={GROUND_LEVEL} receiveShadow onclick={onground}>
  <T.CircleGeometry args={[LAWN_SIZE / 2, 64]} />
  <T.MeshStandardMaterial color={COLORS.lawn} {...lawn} />
</T.Mesh>
