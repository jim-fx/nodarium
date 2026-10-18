<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import { HTML, useCursor } from '@threlte/extras';
  import { DoubleSide, type Group, MeshStandardMaterial, type Texture } from 'three';
  import { PLANT_ISLAND_RADIUS } from './config';
  import { createPlantMesh, disposePlantMesh } from './plant-mesh';
  import { createRandom } from './random';
  import { createMoundGeometry } from './rock-geometry';
  import type { Surface } from './textures';

  type Props = {
    id: number;
    title: string;
    position: { x: number; z: number };
    /** undefined while the project is executed, null if it failed */
    result: Int32Array | null | undefined;
    stone: Surface;
    color: string;
    /** texture of the moss cushion at the root, no cushion without one */
    cushion?: Texture;
    focused: boolean;
    onfocus: (target: { x: number; y: number; z: number; height: number }) => void;
  };
  const { id, title, position, result, stone, color, cushion, focused, onfocus }: Props = $props();

  const GROW_DURATION = 3.5;
  const STONE = '#c4c4c4';
  const MOSS = '#a9b886';

  // the id of a plant never changes, the key of the each block is the id
  // svelte-ignore state_referenced_locally
  const random = createRandom(id);
  // every plant gets a slightly different shade of the color of the style
  const shade = [(random() - 0.5) * 0.06, (random() - 0.5) * 0.1, (random() - 0.5) * 0.08] as const;
  const material = new MeshStandardMaterial({ roughness: 0.75, side: DoubleSide });
  $effect(() => {
    material.color.set(color).offsetHSL(...shade);
  });
  const rotation = random() * Math.PI * 2;
  // svelte-ignore state_referenced_locally
  const mound = createMoundGeometry(id);
  $effect(() => () => mound.dispose());

  const mesh = $derived(result ? createPlantMesh(result, material) : undefined);
  $effect(() => {
    const current = mesh;
    return () => current && disposePlantMesh(current.plant);
  });

  const failed = $derived(result === null || (result && !mesh));
  const height = $derived(mesh?.height ?? (failed ? 0.45 : 0));

  let grown = $state(0);
  let group = $state<Group>();
  useTask((delta) => {
    if (!group || (!mesh && !failed) || grown >= 1) return;
    grown = Math.min(1, grown + delta / GROW_DURATION);
    const eased = 1 - Math.pow(1 - grown, 3);
    group.scale.setScalar(Math.max(eased, 0.001));
  });

  let hovered = $state(false);
  const { onPointerEnter, onPointerLeave } = useCursor();
</script>

<T.Group position={[position.x, 0, position.z]}>
  <T.Group bind:ref={group} rotation.y={rotation} scale={0.001}>
    {#if mesh}
      <T is={mesh.plant} />
      {#if cushion}
        <!-- a cushion of moss roots the plant in the sand -->
        <T.Mesh geometry={mound} scale={[0.4, 0.13, 0.36]} castShadow receiveShadow>
          <T.MeshStandardMaterial color={MOSS} map={cushion} roughness={1} />
        </T.Mesh>
      {/if}
    {:else if failed}
      <!-- projects that can not be executed are marked with a small cairn -->
      {#each [[0.26, 0.09, 0.07], [0.19, 0.08, 0.21], [0.12, 0.07, 0.33]] as [r, h, y], i (i)}
        <T.Mesh
          position.y={y}
          rotation.y={i * 1.3}
          scale={[r, h, r * 0.85]}
          castShadow
          receiveShadow
        >
          <T.SphereGeometry args={[1, 24, 16]} />
          <T.MeshStandardMaterial color={STONE} {...stone} />
        </T.Mesh>
      {/each}
    {/if}
  </T.Group>

  {#if mesh || failed}
    <!-- invisible objects are still hit by the raycaster, but stay out of every render pass -->
    <T.Mesh
      visible={false}
      position.y={height / 2}
      onpointerenter={() => {
        hovered = true;
        onPointerEnter();
      }}
      onpointerleave={() => {
        hovered = false;
        onPointerLeave();
      }}
      onclick={(event: { stopPropagation: () => void }) => {
        event.stopPropagation();
        onfocus({ x: position.x, y: height / 2, z: position.z, height });
      }}
    >
      <T.CylinderGeometry
        args={[PLANT_ISLAND_RADIUS * 0.8, PLANT_ISLAND_RADIUS * 0.8, Math.max(height, 0.3), 16]}
      />
    </T.Mesh>
  {/if}

  {#if hovered || focused}
    <HTML position.y={height + 0.35} center pointerEvents="none">
      <div class="whitespace-nowrap rounded-[3px] border border-neutral-400 bg-neutral-500 px-2.5 py-1 text-xs text-neutral-200">
        {title}
      </div>
    </HTML>
  {/if}
</T.Group>
