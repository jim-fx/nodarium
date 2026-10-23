<script lang="ts">
  import { T } from '@threlte/core';
  import { useDraco, useGltf } from '@threlte/extras';
  import { Box3, type Group, type Mesh, MeshStandardMaterial } from 'three';

  /** the model is 0.64m wide, centered on its origin, standing on y = 0 */
  const MODEL_HALF_WIDTH = 0.33;
  const GAP = 0.25;

  const { bounds }: { bounds?: Box3 } = $props();

  const dracoLoader = useDraco();
  const gltf = useGltf('/models/human.glb', {
    dracoLoader
  });

  let model = $state<Group>();
  const material = new MeshStandardMaterial({
    color: 0x111111
  });

  const position = $derived.by((): [number, number, number] => {
    if (!bounds || bounds.isEmpty()) return [MODEL_HALF_WIDTH + GAP, 0, 0];
    return [bounds.max.x + GAP + MODEL_HALF_WIDTH, 0, (bounds.min.z + bounds.max.z) / 2];
  });

  $effect(() => {
    if (!model?.children?.length) return;
    (model.children[0] as Mesh).material = material;
  });
</script>

<!-- eslint-disable-next-line svelte/require-store-reactive-access -->
{#await gltf then value}
  {#if value}
    <T is={value.scene} bind:ref={model} {position} />
  {/if}
{/await}
